use crate::core::database::Database;
use crate::core::error::AppError;
use crate::core::fs_utils;
use crate::core::models::*;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

pub struct ProjectScanner;

impl ProjectScanner {
    /// 阶段 1：仅收集元信息（name / description / skill_root 路径），不计算 content_hash。
    /// 必须在几百毫秒内完成，因为前端的"立即可见"UX 依赖它。
    pub fn scan_project_skills_phase1(
        project_root: &str,
    ) -> Result<Vec<ProjectSkillInfo>, AppError> {
        Self::scan_project_root(project_root)
    }

    /// 阶段 2：对 phase1 收齐的 skills 按 skill_root 逐个计算 SHA256 目录 hash，填回 content_hash。
    /// 串行执行（不引 rayon，避免 build 体积 + 复杂度）。原本单次扫描的耗时集中在这里。
    pub fn compute_all_hashes(skills: &mut [ProjectSkillInfo]) -> Result<(), AppError> {
        for skill in skills.iter_mut() {
            if skill.skill_root.as_os_str().is_empty() {
                continue;
            }
            skill.content_hash = fs_utils::hash_directory(&skill.skill_root).ok();
        }
        Ok(())
    }

    /// 兼容旧 API：phase1 + phase2 一气呵成。供 `import_project_skill_to_center` 等需要完整
    /// hash 信息的调用点使用。
    pub fn scan_project_skills(project_root: &str) -> Result<Vec<ProjectSkillInfo>, AppError> {
        let mut skills = Self::scan_project_root(project_root)?;
        Self::compute_all_hashes(&mut skills)?;
        Ok(skills)
    }

    /// 内部共享扫描逻辑：遍历 4 个 agent 目录的 skills / skills-disabled 子目录，
    /// 收集元信息 + skill_root 路径。**不**算 content_hash（由 compute_all_hashes 集中算）。
    fn scan_project_root(project_root: &str) -> Result<Vec<ProjectSkillInfo>, AppError> {
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
            (
                "opencode",
                ".config/opencode/skill",
                ".config/opencode/skill-disabled",
            ),
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

            let skill_md = fs_utils::find_skill_marker(&path);
            let content = skill_md.as_ref().and_then(|p| fs::read_to_string(p).ok());
            let (name, description) = if let Some(content) = content {
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
                (name, desc)
            } else {
                (name_str.to_string(), String::new())
            };

            skills.push(ProjectSkillInfo {
                name,
                description,
                relative_path: name_str.to_string(),
                agent: agent.to_string(),
                enabled,
                content_hash: None, // phase1 不算 hash；phase2 在 command 内集中算
                in_center: false,
                center_skill_id: None,
                sync_status: SyncHealthStatus::ProjectOnly,
                skill_root: path,
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
                    } else if pskill.content_hash.as_deref() == center_skill.content_hash.as_deref()
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
            let hash_match = center_skills
                .iter()
                .find(|s| s.content_hash.as_deref() == Some(hash));
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
            .join(
                &pskill
                    .agent
                    .replace("claude-code", ".claude")
                    .replace("opencode", ".config/opencode"),
            )
            .join("skills")
            .join(&pskill.relative_path);

        if !fs_utils::is_valid_skill_dir(&source_path) {
            // Try disabled directory
            let disabled_path = Path::new(project_root)
                .join(&pskill.agent.replace("claude-code", ".claude"))
                .join("skills-disabled")
                .join(&pskill.relative_path);
            if fs_utils::is_valid_skill_dir(&disabled_path) {
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
            source_revision: None,
            source_remote_revision: None,
            source_update_status: Default::default(),
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
        _database: &Database,
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

        let target = Path::new(project_root)
            .join(agent_skills_dir)
            .join(skill_name);
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
    use std::path::PathBuf;
    use tempfile::TempDir;

    #[test]
    fn test_scan_empty_project() {
        let temp = TempDir::new().unwrap();
        let skills = ProjectScanner::scan_project_skills(&temp.path().to_string_lossy()).unwrap();
        assert!(skills.is_empty());
    }

    #[test]
    fn test_scan_project_with_skill() {
        let temp = TempDir::new().unwrap();
        let skill_dir = temp
            .path()
            .join(".claude")
            .join("skills")
            .join("test-skill");
        fs::create_dir_all(&skill_dir).unwrap();
        fs::write(
            skill_dir.join("SKILL.md"),
            "---\nname: test-skill\ndescription: A test skill\n---\n# Body",
        )
        .unwrap();

        let skills = ProjectScanner::scan_project_skills(&temp.path().to_string_lossy()).unwrap();
        assert_eq!(skills.len(), 1);
        assert_eq!(skills[0].name, "test-skill");
        assert_eq!(skills[0].agent, "claude-code");
        assert!(skills[0].enabled);
    }

    #[test]
    fn test_scan_disabled_skill() {
        let temp = TempDir::new().unwrap();
        let disabled_dir = temp
            .path()
            .join(".claude")
            .join("skills-disabled")
            .join("disabled-skill");
        fs::create_dir_all(&disabled_dir).unwrap();
        fs::write(
            disabled_dir.join("SKILL.md"),
            "---\nname: disabled-skill\n---",
        )
        .unwrap();

        let skills = ProjectScanner::scan_project_skills(&temp.path().to_string_lossy()).unwrap();
        assert_eq!(skills.len(), 1);
        assert_eq!(skills[0].name, "disabled-skill");
        assert!(!skills[0].enabled);
    }

    #[test]
    fn test_scan_phase1_skips_hash() {
        // 根因 1 性能瓶颈回归：phase1 收集元信息时绝不能调 hash_directory。
        // 当前 read_skills_from_dir 同步算 hash，大目录会耗时 100ms+。phase1 必须 < 200ms。
        let temp = TempDir::new().unwrap();
        let skill_dir = temp
            .path()
            .join(".claude")
            .join("skills")
            .join("slow-skill");
        fs::create_dir_all(&skill_dir).unwrap();
        fs::write(
            skill_dir.join("SKILL.md"),
            "---\nname: slow-skill\ndescription: x\n---\n# body",
        )
        .unwrap();
        // 加 99 个大文件模拟大 skill，让 hash_directory 显著变慢（>200ms）
        for i in 0..99 {
            fs::write(
                skill_dir.join(format!("file_{i:03}.txt")),
                "x".repeat(10_000),
            )
            .unwrap();
        }

        let start = std::time::Instant::now();
        let skills =
            ProjectScanner::scan_project_skills_phase1(&temp.path().to_string_lossy()).unwrap();
        let elapsed = start.elapsed();

        assert_eq!(skills.len(), 1, "phase1 必须返回该 skill");
        assert_eq!(skills[0].name, "slow-skill");
        assert!(
            skills[0].content_hash.is_none(),
            "phase1 不应该算 hash（content_hash 应为 None）"
        );
        assert!(
            elapsed < std::time::Duration::from_millis(200),
            "phase1 耗时 {elapsed:?} 超 200ms 上限（当前实现会同步算 hash，必超时）"
        );
    }

    #[test]
    fn test_scan_phase2_fills_hashes() {
        // 阶段 4 第二阶段：拿到 phase1 列表后，compute_all_hashes 必须填回 content_hash。
        let temp = TempDir::new().unwrap();
        let skill_dir = temp
            .path()
            .join(".claude")
            .join("skills")
            .join("phased-skill");
        fs::create_dir_all(&skill_dir).unwrap();
        fs::write(
            skill_dir.join("SKILL.md"),
            "---\nname: phased-skill\ndescription: y\n---\n# body",
        )
        .unwrap();

        let mut skills =
            ProjectScanner::scan_project_skills_phase1(&temp.path().to_string_lossy()).unwrap();
        assert_eq!(skills.len(), 1);
        assert!(skills[0].content_hash.is_none());

        ProjectScanner::compute_all_hashes(&mut skills).unwrap();

        assert!(
            skills[0].content_hash.is_some(),
            "phase2 之后 content_hash 必须被填回"
        );
        assert!(
            !skills[0].content_hash.as_ref().unwrap().is_empty(),
            "content_hash 不能是空字符串"
        );
    }

    #[test]
    fn test_scan_phase1_serializes_path_internally() {
        // 验证 skill_root 字段不进 JSON 序列化（前端拿不到绝对路径）
        let temp = TempDir::new().unwrap();
        let skill_dir = temp
            .path()
            .join(".claude")
            .join("skills")
            .join("private-skill");
        fs::create_dir_all(&skill_dir).unwrap();
        fs::write(
            skill_dir.join("SKILL.md"),
            "---\nname: private-skill\n---\n",
        )
        .unwrap();

        let skills =
            ProjectScanner::scan_project_skills_phase1(&temp.path().to_string_lossy()).unwrap();

        let json = serde_json::to_string(&skills[0]).unwrap();
        assert!(
            !json.contains("skill_root"),
            "skill_root 必须 #[serde(skip_serializing)]，前端不能拿到内部路径: {json}"
        );
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
                skill_root: PathBuf::new(),
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
                skill_root: PathBuf::new(),
            },
        ];

        let health = ProjectScanner::compute_sync_health(&skills);
        assert_eq!(health.in_sync, 1);
        assert_eq!(health.project_only, 1);
    }
}
