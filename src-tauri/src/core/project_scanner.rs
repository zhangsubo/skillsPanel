use crate::core::error::AppError;
use crate::core::fs_utils;
use crate::core::models::*;
use crate::core::database::Database;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

pub struct ProjectScanner;

impl ProjectScanner {
    pub fn scan_project_skills(
        project_root: &str,
    ) -> Result<Vec<ProjectSkillInfo>, AppError> {
        let root = Path::new(project_root);
        if !root.exists() {
            return Err(AppError::Validation(format!(
                "Project root does not exist: {}",
                project_root
            )));
        }

        let mut skills = Vec::new();

        // Scan known agent skill directories
        let agent_configs = vec![
            ("claude-code", ".claude/skills", ".claude/skills-disabled"),
            ("cursor", ".cursor/skills", ".cursor/skills-disabled"),
            ("opencode", ".config/opencode/skill", ".config/opencode/skill-disabled"),
            ("codex", ".codex/skills", ".codex/skills-disabled"),
        ];

        for (agent, skills_dir_rel, disabled_dir_rel) in &agent_configs {
            let skills_dir = root.join(skills_dir_rel);
            let disabled_dir = root.join(disabled_dir_rel);

            if skills_dir.exists() {
                Self::read_skills_from_dir(&skills_dir, true, agent, &mut skills);
            }
            if disabled_dir.exists() {
                Self::read_skills_from_dir(&disabled_dir, false, agent, &mut skills);
            }
        }

        Ok(skills)
    }

    fn read_skills_from_dir(
        dir: &Path,
        enabled: bool,
        agent: &str,
        skills: &mut Vec<ProjectSkillInfo>,
    ) {
        if !dir.exists() {
            return;
        }

        let entries = match fs::read_dir(dir) {
            Ok(e) => e,
            Err(_) => return,
        };

        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }

            let file_name = entry.file_name();
            let name_str = file_name.to_string_lossy();
            if name_str.starts_with('.') {
                continue;
            }

            let skill_md = path.join("SKILL.md");
            let content = fs::read_to_string(&skill_md).ok();
            let (name, description, content_hash) = if let Some(content) = content {
                let (fm, _) = fs_utils::parse_frontmatter(&content).unwrap_or_default();
                let name = fm
                    .get("name")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| name_str.to_string());
                let desc = fm
                    .get("description")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let hash = fs_utils::hash_directory(&path).ok();
                (name, desc, hash)
            } else {
                (name_str.to_string(), String::new(), None)
            };

            skills.push(ProjectSkillInfo {
                name,
                description,
                relative_path: name_str.to_string(),
                agent: agent.to_string(),
                enabled,
                content_hash,
                in_center: false,
                center_skill_id: None,
                sync_status: SyncHealthStatus::ProjectOnly,
            });
        }
    }

    pub fn classify_sync_status(
        project_skills: &mut Vec<ProjectSkillInfo>,
        center_skills: &[Skill],
    ) {
        for pskill in project_skills.iter_mut() {
            let matched = Self::find_best_center_match(pskill, center_skills);

            match matched {
                None => {
                    pskill.in_center = false;
                    pskill.sync_status = SyncHealthStatus::ProjectOnly;
                }
                Some(center_skill) => {
                    pskill.in_center = true;
                    pskill.center_skill_id = Some(center_skill.id.clone());

                    if pskill.content_hash.is_none() || center_skill.mtime_ms == 0 {
                        pskill.sync_status = SyncHealthStatus::InSync;
                    } else if pskill.content_hash.as_deref()
                        == center_skill.content_hash.as_deref()
                    {
                        pskill.sync_status = SyncHealthStatus::InSync;
                    } else {
                        pskill.sync_status = SyncHealthStatus::Diverged;
                    }
                }
            }
        }
    }

    fn find_best_center_match<'a>(
        pskill: &ProjectSkillInfo,
        center_skills: &'a [Skill],
    ) -> Option<&'a Skill> {
        // Priority 1: name match
        let name_match = center_skills.iter().find(|s| s.name == pskill.name);
        if let Some(skill) = name_match {
            return Some(skill);
        }

        // Priority 2: content hash match
        if let Some(ref hash) = pskill.content_hash {
            let hash_match = center_skills.iter().find(|s| {
                s.content_hash.as_deref() == Some(hash)
            });
            if let Some(skill) = hash_match {
                return Some(skill);
            }
        }

        None
    }

    pub fn compute_sync_health(skills: &[ProjectSkillInfo]) -> SyncHealthDto {
        let mut health = SyncHealthDto::default();

        for skill in skills {
            match skill.sync_status {
                SyncHealthStatus::InSync => health.in_sync += 1,
                SyncHealthStatus::CenterNewer => health.center_newer += 1,
                SyncHealthStatus::ProjectNewer => health.project_newer += 1,
                SyncHealthStatus::Diverged => health.diverged += 1,
                SyncHealthStatus::ProjectOnly => health.project_only += 1,
                SyncHealthStatus::CenterOnly => health.center_only += 1,
            }
        }

        health
    }

    pub fn import_project_skill_to_center(
        database: &Database,
        project_root: &str,
        skill_name: &str,
        library: &crate::core::library::SkillLibrary,
    ) -> Result<String, AppError> {
        let skills = Self::scan_project_skills(project_root)?;
        let pskill = skills
            .iter()
            .find(|s| s.name == skill_name)
            .ok_or_else(|| AppError::SkillNotFound(skill_name.to_string()))?;

        let source_path = Path::new(project_root)
            .join(&pskill.agent.replace("claude-code", ".claude").replace("opencode", ".config/opencode"))
            .join("skills")
            .join(&pskill.relative_path);

        if !source_path.join("SKILL.md").exists() {
            // Try disabled directory
            let disabled_path = Path::new(project_root)
                .join(&pskill.agent.replace("claude-code", ".claude"))
                .join("skills-disabled")
                .join(&pskill.relative_path);
            if disabled_path.join("SKILL.md").exists() {
                return Self::install_to_center(&disabled_path, &pskill.name, database, library);
            }
            return Err(AppError::InvalidSkill(format!(
                "Skill '{}' not found in project",
                skill_name
            )));
        }

        Self::install_to_center(&source_path, &pskill.name, database, library)
    }

    fn install_to_center(
        source: &Path,
        name: &str,
        database: &Database,
        library: &crate::core::library::SkillLibrary,
    ) -> Result<String, AppError> {
        let skill_id = crate::core::library::SkillLibrary::compute_skill_id(name, source);

        let skill = Skill {
            id: skill_id.clone(),
            name: name.to_string(),
            path_hash: crate::core::library::SkillLibrary::compute_path_hash(source),
            library_path: String::new(),
            original_source_path: Some(source.to_string_lossy().to_string()),
            original_git_url: None,
            original_git_subpath: None,
            group: "project".to_string(),
            description: String::new(),
            frontmatter: HashMap::new(),
            created_at: chrono::Utc::now().to_rfc3339(),
            mtime_ms: 0,
            source_type: SkillSourceType::LocalFolder,
            is_deleted: false,
            content_hash: None,
        };

        let dest = if library.skill_exists(name) {
            library.add_skill_with_overwrite(source, name)?
        } else {
            library.add_skill(source, name)?
        };

        let repo = crate::core::database::SkillsRepository::new(database);
        repo.upsert(&skill)?;
        repo.mark_installed(&skill_id)?;

        Ok(dest.to_string_lossy().to_string())
    }

    pub fn export_center_skill_to_project(
        database: &Database,
        skill_name: &str,
        project_root: &str,
        agent: &str,
        library: &crate::core::library::SkillLibrary,
    ) -> Result<(), AppError> {
        let center_path = library.skill_path(skill_name);
        if !center_path.exists() {
            return Err(AppError::SkillNotFound(skill_name.to_string()));
        }

        let agent_skills_dir = match agent {
            "claude-code" => ".claude/skills",
            "cursor" => ".cursor/skills",
            "opencode" => ".config/opencode/skill",
            "codex" => ".codex/skills",
            _ => return Err(AppError::Validation(format!("Unknown agent: {}", agent))),
        };

        let target = Path::new(project_root).join(agent_skills_dir).join(skill_name);
        if target.exists() {
            return Err(AppError::Conflict(format!(
                "Skill '{}' already exists in project for agent '{}'",
                skill_name, agent
            )));
        }

        fs::create_dir_all(target.parent().unwrap())?;
        crate::core::linker::Linker::copy_skill(&center_path, &target)?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_scan_empty_project() {
        let temp = TempDir::new().unwrap();
        let skills = ProjectScanner::scan_project_skills(
            &temp.path().to_string_lossy(),
        )
        .unwrap();
        assert!(skills.is_empty());
    }

    #[test]
    fn test_scan_project_with_skill() {
        let temp = TempDir::new().unwrap();
        let skill_dir = temp.path().join(".claude").join("skills").join("test-skill");
        fs::create_dir_all(&skill_dir).unwrap();
        fs::write(
            skill_dir.join("SKILL.md"),
            "---\nname: test-skill\ndescription: A test skill\n---\n# Body",
        )
        .unwrap();

        let skills = ProjectScanner::scan_project_skills(
            &temp.path().to_string_lossy(),
        )
        .unwrap();
        assert_eq!(skills.len(), 1);
        assert_eq!(skills[0].name, "test-skill");
        assert_eq!(skills[0].agent, "claude-code");
        assert!(skills[0].enabled);
    }

    #[test]
    fn test_scan_disabled_skill() {
        let temp = TempDir::new().unwrap();
        let disabled_dir = temp.path().join(".claude").join("skills-disabled").join("disabled-skill");
        fs::create_dir_all(&disabled_dir).unwrap();
        fs::write(
            disabled_dir.join("SKILL.md"),
            "---\nname: disabled-skill\n---",
        )
        .unwrap();

        let skills = ProjectScanner::scan_project_skills(
            &temp.path().to_string_lossy(),
        )
        .unwrap();
        assert_eq!(skills.len(), 1);
        assert_eq!(skills[0].name, "disabled-skill");
        assert!(!skills[0].enabled);
    }

    #[test]
    fn test_sync_health_computation() {
        let skills = vec![
            ProjectSkillInfo {
                name: "synced".into(),
                description: "".into(),
                relative_path: "synced".into(),
                agent: "claude-code".into(),
                enabled: true,
                content_hash: Some("abc".into()),
                in_center: true,
                center_skill_id: Some("1".into()),
                sync_status: SyncHealthStatus::InSync,
            },
            ProjectSkillInfo {
                name: "only".into(),
                description: "".into(),
                relative_path: "only".into(),
                agent: "claude-code".into(),
                enabled: true,
                content_hash: None,
                in_center: false,
                center_skill_id: None,
                sync_status: SyncHealthStatus::ProjectOnly,
            },
        ];

        let health = ProjectScanner::compute_sync_health(&skills);
        assert_eq!(health.in_sync, 1);
        assert_eq!(health.project_only, 1);
    }
}