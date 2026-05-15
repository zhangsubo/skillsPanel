use crate::core::error::AppError;
use crate::core::fs_utils;
use crate::core::models::*;
use crate::core::library::SkillLibrary;
use crate::core::database::{Database, SkillsRepository};
use crate::core::git_url_parser::GitUrlParser;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;

pub enum SkillSource {
    Folder(PathBuf),
    Zip(PathBuf),
    ScanEntry(PathBuf),
    Git { url: String, subpath: Option<String> },
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
}

pub struct SkillEngine;

impl SkillEngine {
    pub fn normalize_source(source: SkillSource) -> Result<(PathBuf, Option<tempfile::TempDir>), AppError> {
        match source {
            SkillSource::Folder(path) => {
                if !path.exists() {
                    return Err(AppError::InvalidSkill(format!(
                        "Source folder does not exist: {}", path.display()
                    )));
                }
                Ok((path, None))
            }
            SkillSource::Zip(path) => {
                if !path.exists() {
                    return Err(AppError::InvalidSkill(format!(
                        "ZIP file does not exist: {}", path.display()
                    )));
                }
                let temp_dir = tempfile::tempdir()?;
                let extract_root = temp_dir.path().join("extracted");
                fs::create_dir_all(&extract_root)?;
                let extracted = Self::extract_zip(&path, &extract_root)?;
                
                let skill_dirs = Self::find_skill_dirs(&extracted, &[], true);
                let skill_dir = skill_dirs.into_iter().next().ok_or_else(|| {
                    AppError::InvalidSkill(format!(
                        "No valid skill directory found in zip archive: {}",
                        path.display()
                    ))
                })?;
                
                Ok((skill_dir, Some(temp_dir)))
            }
            SkillSource::ScanEntry(path) => {
                if !path.exists() {
                    return Err(AppError::InvalidSkill(format!(
                        "Scan entry path does not exist: {}", path.display()
                    )));
                }
                Ok((path, None))
            }
SkillSource::Git { url, subpath } => {
                let temp_dir = tempfile::tempdir()?;
                let clone_root = temp_dir.path().join("repo");

                let parsed = GitUrlParser::parse_git_source(&url);
                let final_subpath = subpath.or(parsed.subpath);

                Self::clone_git_repo(&parsed.clone_url, &clone_root)?;

                let skill_dir = if let Some(sub) = final_subpath {
                    let full_path = clone_root.join(&sub);
                    if !full_path.exists() {
                        return Err(AppError::InvalidSkill(format!(
                            "Subpath '{}' not found in cloned repository", sub
                        )));
                    }
                    full_path
                } else {
                    clone_root
                };

                Ok((skill_dir, Some(temp_dir)))
            }
        }
    }

    pub fn validate_skill_dir(skill_dir: &Path) -> Result<PathBuf, AppError> {
        let skill_md = skill_dir.join("SKILL.md");
        if !skill_md.exists() {
            return Err(AppError::InvalidSkill(format!(
                "No SKILL.md found in {}", skill_dir.display()
            )));
        }
        Ok(skill_md)
    }

    pub fn parse_skill_metadata(skill_dir: &Path, source_type: SkillSourceType) -> Result<SkillMetadata, AppError> {
        let skill_md = Self::validate_skill_dir(skill_dir)?;
        let content = fs::read_to_string(&skill_md)?;
        
        let (frontmatter, body) = Self::parse_frontmatter(&content)
            .ok_or_else(|| AppError::InvalidSkill("Invalid or missing YAML frontmatter".into()))?;
        
        let name = frontmatter.get("name")
            .and_then(|v| v.as_str())
            .map(String::from)
            .or_else(|| skill_dir.file_name().map(|n| n.to_string_lossy().into_owned()))
            .unwrap_or_default();
        
        let description = frontmatter.get("description")
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
        let skill_id = SkillLibrary::compute_skill_id(&metadata.name, &metadata.skill_root);
        let path_hash = SkillLibrary::compute_path_hash(&metadata.skill_root);
        let library_path = library.skill_path(&metadata.name);
        
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
        })
    }

    pub fn persist_skill(
        skill: &Skill,
        database: &Database,
    ) -> Result<(), AppError> {
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
            return Err(AppError::Conflict("同名 skill 已存在".to_string()));
        }
        library.add_skill(&metadata.skill_root, &metadata.name)
    }

    pub fn install_skill(
        source: SkillSource,
        library: &SkillLibrary,
        database: &Database,
        name_override: Option<String>,
    ) -> Result<InstallResult, AppError> {
        let source_type = match &source {
            SkillSource::Git { .. } => SkillSourceType::Git,
            SkillSource::Zip(_) => SkillSourceType::LocalZip,
            _ => SkillSourceType::LocalFolder,
        };
        
        let (git_url, git_subpath) = match &source {
            SkillSource::Git { url, subpath } => {
                let (repo_url, parsed_subpath) = Self::parse_git_url(url);
                let final_subpath = subpath.clone().or(parsed_subpath);
                (Some(repo_url), final_subpath)
            },
            _ => (None, None),
        };
        
        let (skill_dir, _temp_dir) = Self::normalize_source(source)?;
        
        let mut metadata = Self::parse_skill_metadata(&skill_dir, source_type)?;
        
        if let Some(name) = name_override {
            if !name.is_empty() {
                metadata.name = name;
            }
        }
        
        let mut skill = Self::create_skill_from_metadata(&metadata, library)?;
        skill.original_git_url = git_url;
        skill.original_git_subpath = git_subpath;
        
        Self::persist_skill(&skill, database)?;
        
        let library_path = Self::sync_to_library(&metadata, library)?;
        
        Ok(InstallResult {
            skill_id: skill.id,
            library_path,
            source_type: skill.source_type,
            linked_tools: Vec::new(),
        })
    }

    pub fn scan_skill_metadata(
        skill_dir: &Path,
        group: &str,
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

    pub fn parse_frontmatter(content: &str) -> Option<(std::collections::HashMap<String, serde_json::Value>, String)> {
        fs_utils::parse_frontmatter(content)
    }

    pub fn extract_zip(zip_path: &Path, dest: &Path) -> Result<PathBuf, AppError> {
        fs_utils::extract_zip(zip_path, dest)
    }

    pub fn is_zip_file(path: &Path) -> bool {
        fs_utils::is_zip_file(path)
    }

    pub fn clone_git_repo(url: &str, dest: &Path) -> Result<(), AppError> {
        let normalized_url = Self::normalize_git_url(url);
        
        git2::Repository::clone(&normalized_url, dest)
            .map_err(|e| AppError::Git(format!("Failed to clone '{}': {}", normalized_url, e)))?;
        
        Ok(())
    }

    pub fn clone_git_repo_with_cancel(
        url: &str,
        dest: &Path,
        cancel: &std::sync::atomic::AtomicBool,
    ) -> Result<(), AppError> {
        let normalized_url = Self::normalize_git_url(url);

        if cancel.load(std::sync::atomic::Ordering::SeqCst) {
            return Err(AppError::Cancelled);
        }

        let mut callbacks = git2::RemoteCallbacks::new();
        callbacks.transfer_progress(move |_stats| {
            !cancel.load(std::sync::atomic::Ordering::SeqCst)
        });

        let mut fetch_opts = git2::FetchOptions::new();
        fetch_opts.remote_callbacks(callbacks);

        let mut builder = git2::build::RepoBuilder::new();
        builder.fetch_options(fetch_opts);

        builder.clone(&normalized_url, dest)
            .map_err(|e| AppError::Git(format!("Failed to clone '{}': {}", normalized_url, e)))?;

        Ok(())
    }

    pub fn normalize_git_url(url: &str) -> String {
        let trimmed = url.trim();
        
        if trimmed.starts_with("https://") || trimmed.starts_with("http://") || trimmed.starts_with("git@") {
            if let Some(repo_url) = Self::extract_github_repo_url(trimmed) {
                return repo_url;
            }
            return trimmed.to_string();
        }
        
        if !trimmed.contains('/') {
            return format!("https://github.com/{}", trimmed);
        }
        
        let parts: Vec<&str> = trimmed.split('/').collect();
        if parts.len() == 2 && !parts[0].contains('.') {
            return format!("https://github.com/{}", trimmed);
        }
        
        format!("https://{}", trimmed)
    }

    pub fn extract_github_repo_url(url: &str) -> Option<String> {
        let patterns = [
            "github.com/",
            "gitlab.com/",
            "bitbucket.org/",
        ];
        
        for pattern in &patterns {
            if let Some(pos) = url.find(pattern) {
                let after_domain = &url[pos + pattern.len()..];
                let parts: Vec<&str> = after_domain.split('/').collect();
                
                if parts.len() >= 2 {
                    let owner = parts[0];
                    let repo = parts[1].trim_end_matches(".git");
                    
                    if owner.is_empty() || repo.is_empty() {
                        continue;
                    }
                    
                    let protocol = if url.starts_with("https://") {
                        "https://"
                    } else if url.starts_with("http://") {
                        "http://"
                    } else {
                        "https://"
                    };
                    
                    return Some(format!("{}{}{}/{}", protocol, pattern, owner, repo));
                }
            }
        }
        
        None
    }

    pub fn parse_git_url(url: &str) -> (String, Option<String>) {
        let trimmed = url.trim();
        
        if let Some(repo_url) = Self::extract_github_repo_url(trimmed) {
            let patterns = [
                "github.com/",
                "gitlab.com/",
                "bitbucket.org/",
            ];
            
            for pattern in &patterns {
                if let Some(pos) = trimmed.find(pattern) {
                    let after_domain = &trimmed[pos + pattern.len()..];
                    let parts: Vec<&str> = after_domain.split('/').collect();
                    
                    if parts.len() >= 4 {
                        if parts[2] == "tree" || parts[2] == "blob" {
                            let subpath = parts[4..].join("/");
                            if !subpath.is_empty() {
                                return (repo_url, Some(subpath));
                            }
                        }
                    }
                }
            }
            
            return (repo_url, None);
        }
        
        (Self::normalize_git_url(trimmed), None)
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
    use tempfile::TempDir;
    use std::io::Write;

    #[test]
    fn test_parse_frontmatter_valid() {
        let content = "---\nname: test-skill\ndescription: A test skill\n---\n# Body\n";
        let (frontmatter, body) = SkillEngine::parse_frontmatter(content).unwrap();
        assert_eq!(frontmatter.get("name").unwrap().as_str().unwrap(), "test-skill");
        assert_eq!(frontmatter.get("description").unwrap().as_str().unwrap(), "A test skill");
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
            "---\nname: my-skill\ndescription: Test description\n---\n# Body content"
        ).unwrap();

        let metadata = SkillEngine::parse_skill_metadata(&skill_dir, SkillSourceType::LocalFolder).unwrap();
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
}
