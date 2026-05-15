use crate::core::error::AppError;
use crate::core::fs_utils;
use crate::core::models::*;
use crate::core::config::AppConfig;
use crate::core::library::SkillLibrary;
use crate::core::skill_engine::SkillEngine;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

pub struct Scanner;

impl Scanner {
    pub fn scan_sources(config: &AppConfig, library: &SkillLibrary) -> Result<Vec<SkillWithStatus>, AppError> {
        let mut skills = Vec::new();
        let mut seen_names = HashMap::new();

        for source in &config.sources {
            if !source.enabled {
                continue;
            }
            let source_path = AppConfig::expand_tilde(&source.path);
            if !source_path.exists() {
                continue;
            }
            let skill_dirs = Self::find_skill_dirs(&source_path, &config.exclude_paths, source.recursive);

            for dir in skill_dirs {
                if let Some(skill) = Self::parse_skill_dir(&dir, &source.group, "local-folder", library) {
                    if let Some(existing) = seen_names.get(&skill.skill.name) {
                        if existing != &dir {
                            continue;
                        }
                    }
                    seen_names.insert(skill.skill.name.clone(), dir);
                    skills.push(skill);
                }
            }
        }

        let library_skills = library.list_skills()?;
        for name in library_skills {
            if seen_names.contains_key(&name) {
                continue;
            }
            let lib_path = library.skill_path(&name);
            if let Some(skill) = Self::parse_skill_dir(&lib_path, "library", "local-folder", library) {
                seen_names.insert(name, lib_path);
                skills.push(skill);
            }
        }

        for skill_with_status in &mut skills {
            let skill_path = std::path::Path::new(&skill_with_status.skill.library_path);
            for tool in &config.tools {
                if !tool.enabled {
                    continue;
                }
                let tool_dir = std::path::Path::new(&tool.path);
                let status = crate::core::linker::Linker::check_status(skill_path, tool_dir, &skill_with_status.skill.name);
                skill_with_status.tool_statuses.insert(tool.id.clone(), status);
            }
        }

        Ok(skills)
    }

    pub fn scan_skills(config: &AppConfig, library: &SkillLibrary) -> Result<ScanResult, AppError> {
        use crate::core::resolver::Resolver;

        let skills = Self::scan_sources(config, library)?;
        let total_skills = skills.len();
        let total_tools = config.tools.len();
        let mut linked_count = 0;
        let mut conflict_count = 0;
        let mut blocked_count = 0;

        for skill in &skills {
            for (tool_name, status) in &skill.tool_statuses {
                match status {
                    SkillToolStatus::Linked => linked_count += 1,
                    SkillToolStatus::Wrong | SkillToolStatus::Directory => conflict_count += 1,
                    SkillToolStatus::Blocked => blocked_count += 1,
                    _ => {}
                }
            }
        }

        Ok(ScanResult {
            total_skills,
            total_tools,
            linked_count,
            conflict_count,
            blocked_count,
            skills,
        })
    }

    pub(crate) fn find_skill_dirs(root: &Path, exclude: &[String], recursive: bool) -> Vec<PathBuf> {
        fs_utils::find_skill_dirs(root, exclude, recursive)
    }

    fn parse_skill_dir(dir: &Path, group: &str, source_type: &str, library: &SkillLibrary) -> Option<SkillWithStatus> {
        let skill_md = dir.join("SKILL.md");
        let content = fs::read_to_string(&skill_md).ok()?;
        let (frontmatter, _body) = SkillEngine::parse_frontmatter(&content)?;

        let name = frontmatter.get("name")
            .and_then(|v| v.as_str())
            .map(String::from)
            .or_else(|| dir.file_name().map(|n| n.to_string_lossy().into_owned()))
            .unwrap_or_default();

        let description = frontmatter.get("description")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let path_hash = SkillLibrary::compute_path_hash(dir);
        let id = SkillLibrary::compute_skill_id(&name, dir);

        let mtime = fs::metadata(dir).ok().and_then(|m| m.modified().ok());
        let mtime_ms = mtime.map(|t| t.duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)).unwrap_or(0);

        let library_path = library.skill_path(&name).to_string_lossy().into_owned();

        let skill = Skill {
            id,
            name: name.clone(),
            path_hash,
            library_path,
            original_source_path: Some(dir.to_string_lossy().into_owned()),
            original_git_url: if source_type == "git" { Some(dir.to_string_lossy().into_owned()) } else { None },
            original_git_subpath: None,
            group: group.to_string(),
            description,
            frontmatter,
            created_at: chrono::Utc::now().to_rfc3339(),
            mtime_ms,
            source_type: match source_type {
                "git" => SkillSourceType::Git,
                "local-zip" => SkillSourceType::LocalZip,
                _ => SkillSourceType::LocalFolder,
            },
            is_deleted: false,
            content_hash: None,
        };

        Some(SkillWithStatus {
            skill,
            tool_statuses: HashMap::new(),
            rule_decisions: HashMap::new(),
        })
    }

    pub fn preview_local_install(path: &Path) -> Result<Vec<InstallCandidate>, AppError> {
        if !path.exists() {
            return Err(AppError::Validation(format!("Path does not exist: {}", path.display())));
        }

        let mut candidates = Vec::new();

        if path.is_file() && path.extension().map(|e| e == "zip").unwrap_or(false) {
            let temp_dir = tempfile::tempdir()?;
            let extracted = Self::extract_zip(path, temp_dir.path())?;
            for skill_dir in Self::find_skill_dirs(&extracted, &[], true) {
                if let Some(candidate) = Self::make_candidate(&skill_dir, path.to_string_lossy().as_ref(), "local-zip") {
                    candidates.push(candidate);
                }
            }
        } else if path.is_dir() {
            let skill_md = path.join("SKILL.md");
            if skill_md.exists() {
                if let Some(candidate) = Self::make_candidate(path, path.to_string_lossy().as_ref(), "local-folder") {
                    candidates.push(candidate);
                }
            }
            let sub_skills = Self::find_skill_dirs(path, &["node_modules".into(), ".git".into()], true);
            for skill_dir in sub_skills {
                if skill_dir != path {
                    if let Some(candidate) = Self::make_candidate(&skill_dir, path.to_string_lossy().as_ref(), "local-folder") {
                        candidates.push(candidate);
                    }
                }
            }
        }

        Ok(candidates)
    }

    fn make_candidate(skill_dir: &Path, source_path: &str, source_type: &str) -> Option<InstallCandidate> {
        let skill_md = skill_dir.join("SKILL.md");
        let content = fs::read_to_string(&skill_md).ok()?;
        let (frontmatter, _) = parse_frontmatter(&content)?;

        let detected_name = frontmatter.get("name").and_then(|v| v.as_str()).map(String::from);
        let description = frontmatter.get("description").and_then(|v| v.as_str()).map(String::from);
        let has_name = detected_name.is_some();

        Some(InstallCandidate {
            candidate_id: uuid::Uuid::new_v4().to_string(),
            detected_name,
            user_name_override: None,
            description,
            source_path: source_path.to_string(),
            skill_root: skill_dir.to_string_lossy().into_owned(),
            valid: has_name || frontmatter.contains_key("name"),
            error: if !has_name { Some("SKILL.md must contain a 'name' field in frontmatter".into()) } else { None },
        })
    }

    pub(crate) fn extract_zip(zip_path: &Path, dest: &Path) -> Result<PathBuf, AppError> {
        fs_utils::extract_zip(zip_path, dest)
    }
}

pub fn parse_frontmatter(content: &str) -> Option<(HashMap<String, serde_json::Value>, String)> {
    fs_utils::parse_frontmatter(content)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_parse_frontmatter_valid() {
        let content = "---\nname: test-skill\ndescription: A test skill\n---\n# Body\n";
        let (frontmatter, body) = parse_frontmatter(content).unwrap();
        assert_eq!(frontmatter.get("name").unwrap().as_str().unwrap(), "test-skill");
        assert_eq!(frontmatter.get("description").unwrap().as_str().unwrap(), "A test skill");
        assert_eq!(body, "# Body");
    }

    #[test]
    fn test_parse_frontmatter_no_frontmatter() {
        let content = "# Just a markdown file\n";
        assert!(parse_frontmatter(content).is_none());
    }

    #[test]
    fn test_parse_frontmatter_empty_yaml() {
        let content = "------\n# Body\n";
        let (frontmatter, body) = parse_frontmatter(content).unwrap();
        assert!(frontmatter.is_empty());
        assert_eq!(body, "# Body");
    }

    #[test]
    fn test_find_skill_dirs_non_recursive() {
        let temp = TempDir::new().unwrap();
        let root = temp.path().join("skills-root");
        fs::create_dir(&root).unwrap();

        fs::create_dir(root.join("skill-a")).unwrap();
        fs::write(root.join("skill-a/SKILL.md"), "---\nname: a\n---").unwrap();

        fs::create_dir(root.join("not-a-skill")).unwrap();
        fs::write(root.join("not-a-skill/README.md"), "# readme").unwrap();

        let dirs = Scanner::find_skill_dirs(&root, &[], false);
        assert_eq!(dirs.len(), 1);
        assert!(dirs[0].ends_with("skill-a"));
    }

    #[test]
    fn test_find_skill_dirs_recursive() {
        let temp = TempDir::new().unwrap();
        let root = temp.path().join("skills-root");
        fs::create_dir(&root).unwrap();

        fs::create_dir_all(root.join("sub/skill-b")).unwrap();
        fs::write(root.join("sub/skill-b/SKILL.md"), "---\nname: b\n---").unwrap();

        let dirs = Scanner::find_skill_dirs(&root, &[], true);
        assert_eq!(dirs.len(), 1);
        assert!(dirs[0].ends_with("skill-b"));
    }

    #[test]
    fn test_find_skill_dirs_excludes() {
        let temp = TempDir::new().unwrap();
        let root = temp.path().join("skills-root");
        fs::create_dir(&root).unwrap();

        fs::create_dir_all(root.join("node_modules/skill-c")).unwrap();
        fs::write(root.join("node_modules/skill-c/SKILL.md"), "---\nname: c\n---").unwrap();

        fs::create_dir(root.join("skill-d")).unwrap();
        fs::write(root.join("skill-d/SKILL.md"), "---\nname: d\n---").unwrap();

        let dirs = Scanner::find_skill_dirs(&root, &["node_modules".into()], true);
        assert_eq!(dirs.len(), 1);
        assert!(dirs[0].ends_with("skill-d"));
    }

    #[test]
    fn test_make_candidate_valid() {
        let temp = TempDir::new().unwrap();
        let dir = temp.path().join("my-skill");
        fs::create_dir(&dir).unwrap();
        fs::write(dir.join("SKILL.md"), "---\nname: my-skill\ndescription: desc\n---").unwrap();

        let candidate = Scanner::make_candidate(&dir, "/source", "local-folder").unwrap();
        assert_eq!(candidate.detected_name, Some("my-skill".to_string()));
        assert_eq!(candidate.description, Some("desc".to_string()));
        assert!(candidate.valid);
        assert!(candidate.error.is_none());
    }

    #[test]
    fn test_make_candidate_missing_name() {
        let temp = TempDir::new().unwrap();
        let dir = temp.path().join("no-name-skill");
        fs::create_dir(&dir).unwrap();
        fs::write(dir.join("SKILL.md"), "---\ndescription: no name\n---").unwrap();

        let candidate = Scanner::make_candidate(&dir, "/source", "local-folder").unwrap();
        assert!(!candidate.valid);
        assert!(candidate.error.is_some());
    }

    #[test]
    fn test_preview_local_install_dir() {
        let temp = TempDir::new().unwrap();
        let skill_dir = temp.path().join("test-skill");
        fs::create_dir(&skill_dir).unwrap();
        fs::write(skill_dir.join("SKILL.md"), "---\nname: test-skill\n---").unwrap();

        let candidates = Scanner::preview_local_install(&skill_dir).unwrap();
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].detected_name, Some("test-skill".to_string()));
    }

    #[test]
    fn test_preview_local_install_nonexistent() {
        let result = Scanner::preview_local_install(Path::new("/nonexistent/path"));
        assert!(result.is_err());
    }
}
