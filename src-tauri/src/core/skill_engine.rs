use crate::core::database::{Database, SkillsRepository};
use crate::core::error::AppError;
use crate::core::fs_utils;
use crate::core::git_url_parser::GitUrlParser;
use crate::core::library::SkillLibrary;
use crate::core::models::*;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

pub enum SkillSource {
    Folder(PathBuf),
    Zip(PathBuf),
    ScanEntry(PathBuf),
    Git {
        url: String,
        subpath: Option<String>,
    },
}

#[derive(Debug, Clone)]
pub struct SkillMetadata {
    pub name: String,
    pub description: String,
    pub frontmatter: HashMap<String, serde_json::Value>,
    pub body: String,
    pub source_path: PathBuf,
    pub skill_root: PathBuf,
    pub source_type: SkillSourceType,
}

#[derive(Debug)]
pub struct InstallResult {
    pub skill_id: String,
    pub library_path: PathBuf,
    pub source_type: SkillSourceType,
    pub linked_tools: Vec<String>,
    pub head_sha: Option<String>,
}

pub struct SkillEngine;

impl SkillEngine {
    pub fn normalize_source(
        source: SkillSource,
    ) -> Result<(PathBuf, Option<tempfile::TempDir>), AppError> {
        Self::normalize_source_with_progress(source, None, None)
    }

    pub fn normalize_source_with_progress(
        source: SkillSource,
        cancel: Option<std::sync::Arc<std::sync::atomic::AtomicBool>>,
        progress_fn: Option<crate::core::git_clone::ProgressFn>,
    ) -> Result<(PathBuf, Option<tempfile::TempDir>), AppError> {
        match source {
            SkillSource::Folder(path) => {
                if !path.exists() {
                    return Err(AppError::InvalidSkill(format!(
                        "Source folder does not exist: {}",
                        path.display()
                    )));
                }
                let resolved = fs_utils::resolve_skill_dir(&path, None)?;
                Ok((resolved, None))
            }
            SkillSource::Zip(path) => {
                if !path.exists() {
                    return Err(AppError::InvalidSkill(format!(
                        "ZIP file does not exist: {}",
                        path.display()
                    )));
                }
                let temp_dir = tempfile::tempdir()?;
                let extract_root = temp_dir.path().join("extracted");
                fs::create_dir_all(&extract_root)?;
                let extracted = Self::extract_zip(&path, &extract_root)?;

                let skill_dir =
                    fs_utils::resolve_skill_dir(&extracted, None).map_err(|e| match e {
                        AppError::InvalidSkill(msg) => AppError::InvalidSkill(format!(
                            "ZIP archive {}: {}",
                            path.display(),
                            msg
                        )),
                        other => other,
                    })?;

                Ok((skill_dir, Some(temp_dir)))
            }
            SkillSource::ScanEntry(path) => {
                if !path.exists() {
                    return Err(AppError::InvalidSkill(format!(
                        "Scan entry path does not exist: {}",
                        path.display()
                    )));
                }
                Ok((path, None))
            }
            SkillSource::Git { url, subpath } => {
                let temp_dir = tempfile::tempdir()?;
                let clone_root = temp_dir.path().join("repo");

                let parsed = GitUrlParser::parse_git_source(&url);

                // Clone with caching support
                let cancel = cancel.unwrap_or_else(|| {
                    std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false))
                });
                crate::core::git_clone::clone_with_cache(
                    &parsed.clone_url,
                    &clone_root,
                    parsed.branch.as_deref(),
                    Some(cancel),
                    progress_fn.as_ref(),
                )?;

                // Resolve tree_path after cloning to disambiguate branch with slashes
                let final_subpath = if let Some(ref tree_path) = parsed.tree_path {
                    let repo = git2::Repository::open(&clone_root)
                        .map_err(|e| AppError::Git(format!("Failed to open cloned repo: {}", e)))?;
                    if let Some((resolved_branch, resolved_subpath)) =
                        GitUrlParser::resolve_tree_path(&repo, tree_path)
                    {
                        // If branch was ambiguous, checkout the resolved branch
                        if parsed.branch.as_ref() != Some(&resolved_branch) {
                            Self::checkout_branch(&clone_root, &resolved_branch)?;
                        }
                        subpath.or(resolved_subpath)
                    } else {
                        subpath.or(parsed.subpath)
                    }
                } else {
                    subpath.or(parsed.subpath)
                };

                let skill_dir = fs_utils::resolve_skill_dir(&clone_root, final_subpath.as_deref())?;

                Ok((skill_dir, Some(temp_dir)))
            }
        }
    }

    pub fn validate_skill_dir(skill_dir: &Path) -> Result<PathBuf, AppError> {
        fs_utils::find_skill_marker(skill_dir).ok_or_else(|| {
            AppError::InvalidSkill(format!(
                "No SKILL.md or skill.md found in {}",
                skill_dir.display()
            ))
        })
    }

    pub fn parse_skill_metadata(
        skill_dir: &Path,
        source_type: SkillSourceType,
    ) -> Result<SkillMetadata, AppError> {
        let skill_md = Self::validate_skill_dir(skill_dir)?;
        let content = fs::read_to_string(&skill_md)?;

        let (frontmatter, body) = Self::parse_frontmatter(&content)
            .ok_or_else(|| AppError::InvalidSkill("Invalid or missing YAML frontmatter".into()))?;

        let raw_name = frontmatter
            .get("name")
            .and_then(|v| v.as_str())
            .map(String::from)
            .or_else(|| {
                skill_dir
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
            })
            .unwrap_or_default();

        let name = fs_utils::sanitize_skill_name(&raw_name).ok_or_else(|| {
            AppError::InvalidSkill(format!(
                "Skill name '{}' is invalid (empty, path traversal, or reserved)",
                raw_name
            ))
        })?;

        let description = frontmatter
            .get("description")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        Ok(SkillMetadata {
            name,
            description,
            frontmatter,
            body,
            source_path: skill_dir.to_path_buf(),
            skill_root: skill_dir.to_path_buf(),
            source_type,
        })
    }

    pub fn create_skill_from_metadata(
        metadata: &SkillMetadata,
        library: &SkillLibrary,
    ) -> Result<Skill, AppError> {
        let library_path = library.skill_path(&metadata.name);
        let skill_id = SkillLibrary::compute_skill_id(&metadata.name, &library_path);
        let path_hash = SkillLibrary::compute_path_hash(&library_path);

        Ok(Skill {
            id: skill_id,
            name: metadata.name.clone(),
            path_hash,
            library_path: library_path.to_string_lossy().to_string(),
            original_source_path: Some(metadata.source_path.to_string_lossy().to_string()),
            original_git_url: None,
            original_git_subpath: None,
            group: "library".to_string(),
            description: metadata.description.clone(),
            frontmatter: metadata.frontmatter.clone(),
            created_at: chrono::Utc::now().to_rfc3339(),
            mtime_ms: 0,
            source_type: metadata.source_type.clone(),
            is_deleted: false,
            content_hash: None,
            source_revision: None,
            source_remote_revision: None,
            source_update_status: Default::default(),
        })
    }

    pub fn persist_skill(skill: &Skill, database: &Database) -> Result<(), AppError> {
        let repo = SkillsRepository::new(database);
        repo.upsert(skill)?;
        repo.mark_installed(&skill.id)?;
        Ok(())
    }

    pub fn sync_to_library(
        metadata: &SkillMetadata,
        library: &SkillLibrary,
    ) -> Result<PathBuf, AppError> {
        if library.skill_exists(&metadata.name) {
            return Err(AppError::Conflict(format!(
                "同名 skill 已存在: '{}' (路径: {})",
                metadata.name,
                library.skill_path(&metadata.name).display()
            )));
        }
        library.add_skill(&metadata.skill_root, &metadata.name)
    }

    pub fn install_skill(
        source: SkillSource,
        library: &SkillLibrary,
        database: &Database,
        name_override: Option<String>,
    ) -> Result<InstallResult, AppError> {
        Self::install_skill_with_progress(source, library, database, name_override, None, None)
    }

    pub fn install_skill_with_progress(
        source: SkillSource,
        library: &SkillLibrary,
        database: &Database,
        name_override: Option<String>,
        cancel: Option<std::sync::Arc<std::sync::atomic::AtomicBool>>,
        progress_fn: Option<crate::core::git_clone::ProgressFn>,
    ) -> Result<InstallResult, AppError> {
        let source_type = match &source {
            SkillSource::Git { .. } => SkillSourceType::Git,
            SkillSource::Zip(_) => SkillSourceType::LocalZip,
            _ => SkillSourceType::LocalFolder,
        };

        let (git_url, git_subpath) = match &source {
            SkillSource::Git { url, subpath } => {
                let parsed = GitUrlParser::parse_git_source(url);
                let final_subpath = subpath.clone().or(parsed.subpath);
                (Some(parsed.clone_url), final_subpath)
            }
            _ => (None, None),
        };

        let (skill_dir, _temp_dir) =
            Self::normalize_source_with_progress(source, cancel, progress_fn)?;

        // Get HEAD SHA for git repos
        let head_sha = if source_type == SkillSourceType::Git {
            crate::core::git_clone::get_head_sha(&skill_dir).ok()
        } else {
            None
        };

        let mut metadata = Self::parse_skill_metadata(&skill_dir, source_type)?;

        if let Some(raw) = name_override {
            if !raw.is_empty() {
                let sanitized = fs_utils::sanitize_skill_name(&raw).ok_or_else(|| {
                    AppError::InvalidSkill(format!(
                        "Override name '{}' is invalid (empty, path traversal, or reserved)",
                        raw
                    ))
                })?;
                metadata.name = sanitized;
            }
        }

        let mut skill = Self::create_skill_from_metadata(&metadata, library)?;
        skill.original_git_url = git_url;
        skill.original_git_subpath = git_subpath;
        skill.source_revision = head_sha.clone();

        Self::persist_skill(&skill, database)?;

        let library_path = Self::sync_to_library(&metadata, library)?;

        Ok(InstallResult {
            skill_id: skill.id,
            library_path,
            source_type: skill.source_type,
            linked_tools: Vec::new(),
            head_sha,
        })
    }

    /// Install all skills found in a git repository (no subpath required).
    ///
    /// Clones once, discovers every skill directory, and installs each one.
    /// Returns the list of install results (one per skill).
    pub fn install_all_git_skills(
        url: &str,
        library: &SkillLibrary,
        database: &Database,
        cancel: Option<std::sync::Arc<std::sync::atomic::AtomicBool>>,
        progress_fn: Option<crate::core::git_clone::ProgressFn>,
        name_filter: Option<&str>,
    ) -> Result<Vec<InstallResult>, AppError> {
        let parsed = GitUrlParser::parse_git_source(url);
        eprintln!("[install_all_git_skills] url={}, clone_url={}, branch={:?}, subpath={:?}, tree_path={:?}",
            url, parsed.clone_url, parsed.branch, parsed.subpath, parsed.tree_path);

        let cancel_token = cancel
            .unwrap_or_else(|| std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)));

        let temp_dir = tempfile::tempdir()?;
        let clone_root = temp_dir.path().join("repo");

        crate::core::git_clone::clone_with_cache(
            &parsed.clone_url,
            &clone_root,
            parsed.branch.as_deref(),
            Some(cancel_token),
            progress_fn.as_ref(),
        )?;

        let head_sha = crate::core::git_clone::get_head_sha(&clone_root).ok();

        // Resolve subpath from tree URL if present
        let base_dir = if let Some(ref tree_path) = parsed.tree_path {
            let repo = git2::Repository::open(&clone_root)
                .map_err(|e| AppError::Git(format!("Failed to open cloned repo: {}", e)))?;
            if let Some((resolved_branch, resolved_subpath)) =
                GitUrlParser::resolve_tree_path(&repo, tree_path)
            {
                if parsed.branch.as_ref() != Some(&resolved_branch) {
                    Self::checkout_branch(&clone_root, &resolved_branch)?;
                }
                if let Some(sub) = resolved_subpath {
                    clone_root.join(sub)
                } else {
                    clone_root.clone()
                }
            } else {
                clone_root.clone()
            }
        } else if let Some(sub) = parsed.subpath {
            clone_root.join(sub)
        } else {
            clone_root.clone()
        };

        // Find all skill directories
        eprintln!("[install_all_git_skills] base_dir={}", base_dir.display());
        let skill_dirs = fs_utils::find_skill_dirs(&base_dir, &[], true);
        eprintln!(
            "[install_all_git_skills] found {} skill dirs",
            skill_dirs.len()
        );
        if skill_dirs.is_empty() {
            return Err(AppError::InvalidSkill(format!(
                "No SKILL.md or skill.md found in {}",
                base_dir.display()
            )));
        }

        let mut results = Vec::new();
        let filtered_dirs: Vec<_> = if let Some(name) = name_filter {
            // Match against frontmatter name AND directory name, since a skill
            // at the repo root has file_name() == "repo" which never matches.
            skill_dirs
                .iter()
                .filter(|d| {
                    let dir_matches = d
                        .file_name()
                        .and_then(|n| n.to_str())
                        .map(|n| n == name)
                        .unwrap_or(false);
                    if dir_matches {
                        return true;
                    }
                    // Try matching against the frontmatter name inside SKILL.md
                    if let Ok(content) =
                        fs::read_to_string(d.join(fs_utils::SKILL_MARKER_CANONICAL))
                            .or_else(|_| fs::read_to_string(d.join(fs_utils::SKILL_MARKER_LEGACY)))
                    {
                        if let Some((fm, _)) = Self::parse_frontmatter(&content) {
                            if let Some(frontmatter_name) = fm.get("name").and_then(|v| v.as_str())
                            {
                                return frontmatter_name == name;
                            }
                        }
                    }
                    false
                })
                .cloned()
                .collect()
        } else {
            skill_dirs.clone()
        };
        if filtered_dirs.is_empty() && name_filter.is_some() {
            let available: Vec<String> = skill_dirs
                .iter()
                .filter_map(|d| {
                    let dir_name = d.file_name().and_then(|n| n.to_str()).map(String::from);
                    let fm_name = fs::read_to_string(d.join(fs_utils::SKILL_MARKER_CANONICAL))
                        .or_else(|_| fs::read_to_string(d.join(fs_utils::SKILL_MARKER_LEGACY)))
                        .ok()
                        .and_then(|c| Self::parse_frontmatter(&c))
                        .and_then(|(fm, _)| {
                            fm.get("name").and_then(|v| v.as_str()).map(String::from)
                        });
                    fm_name.or(dir_name)
                })
                .collect();
            return Err(AppError::InvalidSkill(format!(
                "No skill matching '{}' found. Available: {}",
                name_filter.unwrap(),
                available.join(", ")
            )));
        }

        for skill_dir in &filtered_dirs {
            let source_type = SkillSourceType::Git;
            let mut metadata = Self::parse_skill_metadata(skill_dir, source_type)?;

            // Use directory name as fallback for name
            if metadata.name.is_empty() {
                if let Some(dir_name) = skill_dir.file_name().and_then(|n| n.to_str()) {
                    metadata.name = dir_name.to_string();
                }
            }

            let mut skill = Self::create_skill_from_metadata(&metadata, library)?;
            skill.original_git_url = Some(parsed.clone_url.clone());
            skill.original_git_subpath = skill_dir
                .strip_prefix(&clone_root)
                .ok()
                .map(|p| p.to_string_lossy().replace('\\', "/"));
            skill.source_revision = head_sha.clone();

            Self::persist_skill(&skill, database)?;

            let library_path = Self::sync_to_library(&metadata, library)?;

            results.push(InstallResult {
                skill_id: skill.id,
                library_path,
                source_type: skill.source_type,
                linked_tools: Vec::new(),
                head_sha: head_sha.clone(),
            });
        }

        Ok(results)
    }

    pub fn scan_skill_metadata(
        skill_dir: &Path,
        _group: &str,
        source_type: &str,
    ) -> Result<SkillMetadata, AppError> {
        let source_type_enum = match source_type {
            "git" => SkillSourceType::Git,
            "local-zip" => SkillSourceType::LocalZip,
            _ => SkillSourceType::LocalFolder,
        };

        let mut metadata = Self::parse_skill_metadata(skill_dir, source_type_enum)?;
        metadata.source_path = skill_dir.to_path_buf();

        Ok(metadata)
    }

    pub fn find_skill_dirs(root: &Path, exclude: &[String], recursive: bool) -> Vec<PathBuf> {
        fs_utils::find_skill_dirs(root, exclude, recursive)
    }

    pub fn parse_frontmatter(
        content: &str,
    ) -> Option<(std::collections::HashMap<String, serde_json::Value>, String)> {
        fs_utils::parse_frontmatter(content)
    }

    pub fn extract_zip(zip_path: &Path, dest: &Path) -> Result<PathBuf, AppError> {
        fs_utils::extract_zip(zip_path, dest)
    }

    pub fn is_zip_file(path: &Path) -> bool {
        fs_utils::is_zip_file(path)
    }

    pub fn clone_git_repo(url: &str, dest: &Path) -> Result<(), AppError> {
        let normalized_url = GitUrlParser::normalize_git_url(url);

        git2::Repository::clone(&normalized_url, dest)
            .map_err(|e| AppError::Git(format!("Failed to clone '{}': {}", normalized_url, e)))?;

        Ok(())
    }

    pub fn clone_git_repo_branch(url: &str, dest: &Path, branch: &str) -> Result<(), AppError> {
        // Don't normalize local paths - they should be used as-is
        let clone_url = if std::path::Path::new(url).is_absolute() || url.starts_with("file://") {
            url.to_string()
        } else {
            GitUrlParser::normalize_git_url(url)
        };

        let mut callbacks = git2::RemoteCallbacks::new();
        callbacks.transfer_progress(|_stats| true);

        let mut fetch_opts = git2::FetchOptions::new();
        fetch_opts.remote_callbacks(callbacks);

        let mut builder = git2::build::RepoBuilder::new();
        builder.branch(branch);
        builder.fetch_options(fetch_opts);

        builder.clone(&clone_url, dest).map_err(|e| {
            AppError::Git(format!(
                "Failed to clone '{}' (branch {}): {}",
                clone_url, branch, e
            ))
        })?;

        Ok(())
    }

    fn checkout_branch(repo_path: &Path, branch: &str) -> Result<(), AppError> {
        let repo = git2::Repository::open(repo_path)
            .map_err(|e| AppError::Git(format!("Failed to open repo: {}", e)))?;

        let reference = repo
            .find_branch(&format!("origin/{}", branch), git2::BranchType::Remote)
            .map_err(|e| AppError::Git(format!("Failed to find branch '{}': {}", branch, e)))?;

        repo.set_head(
            reference
                .get()
                .name()
                .unwrap_or(&format!("refs/remotes/origin/{}", branch)),
        )
        .map_err(|e| AppError::Git(format!("Failed to set HEAD: {}", e)))?;

        repo.checkout_head(Some(git2::build::CheckoutBuilder::default().force()))
            .map_err(|e| AppError::Git(format!("Failed to checkout: {}", e)))?;

        Ok(())
    }

    pub fn clone_git_repo_with_cancel(
        url: &str,
        dest: &Path,
        cancel: &std::sync::atomic::AtomicBool,
    ) -> Result<(), AppError> {
        let normalized_url = GitUrlParser::normalize_git_url(url);

        if cancel.load(std::sync::atomic::Ordering::SeqCst) {
            return Err(AppError::Cancelled);
        }

        let mut callbacks = git2::RemoteCallbacks::new();
        callbacks
            .transfer_progress(move |_stats| !cancel.load(std::sync::atomic::Ordering::SeqCst));

        let mut fetch_opts = git2::FetchOptions::new();
        fetch_opts.remote_callbacks(callbacks);

        let mut builder = git2::build::RepoBuilder::new();
        builder.fetch_options(fetch_opts);

        builder
            .clone(&normalized_url, dest)
            .map_err(|e| AppError::Git(format!("Failed to clone '{}': {}", normalized_url, e)))?;

        Ok(())
    }

    pub fn infer_source(path: &Path) -> SkillSource {
        if path.is_file() && Self::is_zip_file(path) {
            SkillSource::Zip(path.to_path_buf())
        } else {
            SkillSource::Folder(path.to_path_buf())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    #[test]
    fn test_parse_frontmatter_valid() {
        let content = "---\nname: test-skill\ndescription: A test skill\n---\n# Body\n";
        let (frontmatter, body) = SkillEngine::parse_frontmatter(content).unwrap();
        assert_eq!(
            frontmatter.get("name").unwrap().as_str().unwrap(),
            "test-skill"
        );
        assert_eq!(
            frontmatter.get("description").unwrap().as_str().unwrap(),
            "A test skill"
        );
        assert_eq!(body, "# Body");
    }

    #[test]
    fn test_parse_frontmatter_no_frontmatter() {
        let content = "# Just a markdown file\n";
        assert!(SkillEngine::parse_frontmatter(content).is_none());
    }

    #[test]
    fn test_validate_skill_dir_valid() {
        let temp = TempDir::new().unwrap();
        let skill_dir = temp.path().join("test-skill");
        fs::create_dir(&skill_dir).unwrap();
        fs::write(skill_dir.join("SKILL.md"), "---\nname: test\n---").unwrap();

        let result = SkillEngine::validate_skill_dir(&skill_dir);
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_skill_dir_missing() {
        let temp = TempDir::new().unwrap();
        let skill_dir = temp.path().join("no-skill");
        fs::create_dir(&skill_dir).unwrap();

        let result = SkillEngine::validate_skill_dir(&skill_dir);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_skill_metadata() {
        let temp = TempDir::new().unwrap();
        let skill_dir = temp.path().join("my-skill");
        fs::create_dir(&skill_dir).unwrap();
        fs::write(
            skill_dir.join("SKILL.md"),
            "---\nname: my-skill\ndescription: Test description\n---\n# Body content",
        )
        .unwrap();

        let metadata =
            SkillEngine::parse_skill_metadata(&skill_dir, SkillSourceType::LocalFolder).unwrap();
        assert_eq!(metadata.name, "my-skill");
        assert_eq!(metadata.description, "Test description");
        assert_eq!(metadata.body, "# Body content");
        assert_eq!(metadata.source_type, SkillSourceType::LocalFolder);
    }

    #[test]
    fn test_find_skill_dirs() {
        let temp = TempDir::new().unwrap();
        let root = temp.path().join("skills-root");
        fs::create_dir(&root).unwrap();

        fs::create_dir(root.join("skill-a")).unwrap();
        fs::write(root.join("skill-a/SKILL.md"), "---\nname: a\n---").unwrap();

        fs::create_dir(root.join("not-a-skill")).unwrap();
        fs::write(root.join("not-a-skill/README.md"), "# readme").unwrap();

        let dirs = SkillEngine::find_skill_dirs(&root, &[], false);
        assert_eq!(dirs.len(), 1);
        assert!(dirs[0].ends_with("skill-a"));
    }

    #[test]
    fn test_extract_zip() {
        let temp = TempDir::new().unwrap();
        let zip_path = temp.path().join("test.zip");

        let file = fs::File::create(&zip_path).unwrap();
        let mut zip = zip::ZipWriter::new(file);
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored);

        zip.start_file("test-skill/SKILL.md", options).unwrap();
        zip.write_all(b"---\nname: test\n---").unwrap();
        zip.finish().unwrap();

        let dest = temp.path().join("extracted");
        fs::create_dir(&dest).unwrap();

        let result = SkillEngine::extract_zip(&zip_path, &dest);
        assert!(result.is_ok());
        assert!(dest.join("test-skill/SKILL.md").exists());
    }

    #[test]
    fn test_is_zip_file() {
        let temp = TempDir::new().unwrap();

        let zip_path = temp.path().join("test.zip");
        fs::write(&zip_path, b"PK\x03\x04").unwrap();
        assert!(SkillEngine::is_zip_file(&zip_path));

        let txt_path = temp.path().join("test.txt");
        fs::write(&txt_path, b"not a zip").unwrap();
        assert!(!SkillEngine::is_zip_file(&txt_path));
    }

    #[test]
    fn test_infer_source() {
        let temp = TempDir::new().unwrap();

        let folder_path = temp.path().join("folder");
        fs::create_dir(&folder_path).unwrap();
        let source = SkillEngine::infer_source(&folder_path);
        assert!(matches!(source, SkillSource::Folder(_)));

        let zip_path = temp.path().join("test.zip");
        fs::write(&zip_path, b"PK\x03\x04").unwrap();
        let source = SkillEngine::infer_source(&zip_path);
        assert!(matches!(source, SkillSource::Zip(_)));
    }

    /// Core regression test: skill ID must be computed from the library path,
    /// not the temp/source path, so that the scanner can find the same skill later.
    #[test]
    fn test_create_skill_id_matches_scanner_id() {
        use crate::core::config::AppConfig;
        use crate::core::models::SyncConfig;

        let temp = TempDir::new().unwrap();
        let config = AppConfig {
            library_path: temp.path().join("library"),
            tools: vec![],
            sources: vec![],
            sync: SyncConfig {
                mode: crate::core::models::SyncMode::Symlink,
            },
            install: crate::core::models::InstallConfig {
                allow_zip: true,
                allow_git: true,
                default_sync_targets: vec![],
            },
            exclude_paths: vec![],
            rules: crate::core::models::RulesConfig::default(),
            deleted_skills: vec![],
            debug_logging: false,
        };
        let library = SkillLibrary::new(&config).unwrap();

        // Simulate a source in a temp directory (like git clone)
        let source_dir = temp.path().join("tmp-clone/my-skill");
        fs::create_dir_all(&source_dir).unwrap();
        fs::write(
            source_dir.join("SKILL.md"),
            "---\nname: my-skill\ndescription: test\n---",
        )
        .unwrap();

        let metadata =
            SkillEngine::parse_skill_metadata(&source_dir, SkillSourceType::Git).unwrap();
        let skill = SkillEngine::create_skill_from_metadata(&metadata, &library).unwrap();

        // The scanner discovers the skill from the library path, not the source path
        let library_path = library.skill_path(&metadata.name);
        let scanner_id = SkillLibrary::compute_skill_id(&metadata.name, &library_path);

        assert_eq!(
            skill.id, scanner_id,
            "Skill ID from install ({}) must match scanner ID ({}) computed from library path",
            skill.id, scanner_id
        );
    }

    /// End-to-end test: install a skill, then simulate a scan — the skill
    /// must remain installed (is_installed = 1) after the scan.
    #[test]
    fn test_install_then_scan_preserves_installed_status() {
        use crate::core::config::AppConfig;
        use crate::core::database::Database;
        use crate::core::models::SyncConfig;

        let temp = TempDir::new().unwrap();
        let config = AppConfig {
            library_path: temp.path().join("library"),
            tools: vec![],
            sources: vec![],
            sync: SyncConfig {
                mode: crate::core::models::SyncMode::Symlink,
            },
            install: crate::core::models::InstallConfig {
                allow_zip: true,
                allow_git: true,
                default_sync_targets: vec![],
            },
            exclude_paths: vec![],
            rules: crate::core::models::RulesConfig::default(),
            deleted_skills: vec![],
            debug_logging: false,
        };
        let library = SkillLibrary::new(&config).unwrap();
        let db = Database::new(&temp.path().join("test.db")).unwrap();

        // 1. Create a skill source and install it
        let source_dir = temp.path().join("source/my-skill");
        fs::create_dir_all(&source_dir).unwrap();
        fs::write(
            source_dir.join("SKILL.md"),
            "---\nname: my-skill\ndescription: test\n---",
        )
        .unwrap();

        let metadata =
            SkillEngine::parse_skill_metadata(&source_dir, SkillSourceType::Git).unwrap();
        let skill = SkillEngine::create_skill_from_metadata(&metadata, &library).unwrap();
        SkillEngine::persist_skill(&skill, &db).unwrap();

        // Verify it's installed
        let repo = crate::core::database::SkillsRepository::new(&db);
        let installed = repo.get_installed().unwrap();
        assert_eq!(
            installed.len(),
            1,
            "Skill should be installed after persist"
        );
        assert_eq!(installed[0].id, skill.id);

        // 2. Simulate a scan: copy skill to library, then scan
        library.add_skill(&source_dir, &metadata.name).unwrap();
        let scan_ts = "2024-06-01T00:00:00Z";

        // The scanner computes the same ID as the install (this is the fix)
        let lib_path = library.skill_path(&metadata.name);
        let scanned_id = SkillLibrary::compute_skill_id(&metadata.name, &lib_path);
        assert_eq!(
            skill.id, scanned_id,
            "IDs must match for scan upsert to work"
        );

        // Upsert the scanned skill (simulating what commands.rs does)
        let scanned_skill = Skill {
            id: scanned_id.clone(),
            name: metadata.name.clone(),
            path_hash: SkillLibrary::compute_path_hash(&lib_path),
            library_path: lib_path.to_string_lossy().to_string(),
            original_source_path: Some(lib_path.to_string_lossy().to_string()),
            original_git_url: None,
            original_git_subpath: None,
            group: "library".to_string(),
            description: metadata.description.clone(),
            frontmatter: metadata.frontmatter.clone(),
            created_at: skill.created_at.clone(),
            mtime_ms: 0,
            source_type: SkillSourceType::LocalFolder,
            is_deleted: false,
            content_hash: None,
            source_revision: None,
            source_remote_revision: None,
            source_update_status: Default::default(),
        };
        repo.upsert_with_scan(&scanned_skill, scan_ts).unwrap();
        repo.mark_missing_as_deleted(scan_ts).unwrap();

        // 3. Verify: skill must still be installed
        let installed_after_scan = repo.get_installed().unwrap();
        assert_eq!(
            installed_after_scan.len(),
            1,
            "Skill must remain installed after scan (got {} installed skills)",
            installed_after_scan.len()
        );
        assert_eq!(installed_after_scan[0].id, skill.id);
    }
}
