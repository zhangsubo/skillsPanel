use crate::core::config::AppConfig;
use crate::core::database::Database;
use crate::core::error::AppError;
use crate::core::models::AuditEntry;
use std::fs;

pub struct Migration;

impl Migration {
    pub fn run_on_startup(db: &Database, config: &AppConfig) -> Result<MigrationResult, AppError> {
        let mut result = MigrationResult::default();

        Self::migrate_config(db, config, &mut result)?;
        Self::migrate_audit_log(db, &mut result)?;
        Self::migrate_tools(db, config, &mut result)?;
        Self::migrate_library_skills(db, config, &mut result)?;

        Ok(result)
    }

    fn migrate_config(
        db: &Database,
        config: &AppConfig,
        result: &mut MigrationResult,
    ) -> Result<(), AppError> {
        let repo = crate::core::database::ConfigRepository::new(db);

        if repo.get("library_path").is_ok() && repo.get("library_path").unwrap().is_some() {
            return Ok(());
        }

        let config_json = serde_json::to_string(config)
            .map_err(|e| AppError::Config(format!("Failed to serialize config: {}", e)))?;
        repo.set("app_config", &config_json)?;
        repo.set("library_path", &config.library_path.to_string_lossy())?;

        let tools_json = serde_json::to_string(&config.tools)
            .map_err(|e| AppError::Config(format!("Failed to serialize tools: {}", e)))?;
        repo.set("tools", &tools_json)?;

        let sources_json = serde_json::to_string(&config.sources)
            .map_err(|e| AppError::Config(format!("Failed to serialize sources: {}", e)))?;
        repo.set("sources", &sources_json)?;

        result.config_migrated = true;
        Ok(())
    }

    fn migrate_audit_log(db: &Database, result: &mut MigrationResult) -> Result<(), AppError> {
        let audit_repo = crate::core::database::AuditRepository::new(db);
        let existing_logs = audit_repo.get_logs(1)?;
        if !existing_logs.is_empty() {
            return Ok(());
        }

        let config_dir = dirs::home_dir()
            .ok_or_else(|| AppError::Config("Cannot find home directory".into()))?
            .join(".skills-panel");
        let audit_path = config_dir.join("audit.json");

        if !audit_path.exists() {
            return Ok(());
        }

        let content = fs::read_to_string(&audit_path)
            .map_err(|e| AppError::Config(format!("Failed to read audit.json: {}", e)))?;
        let entries: Vec<AuditEntry> = serde_json::from_str(&content)
            .map_err(|e| AppError::Config(format!("Failed to parse audit.json: {}", e)))?;

        for entry in &entries {
            audit_repo.log(
                &entry.action,
                &entry.target,
                entry.details.clone(),
                entry.success,
                entry.error.clone(),
            )?;
        }

        result.audit_migrated = entries.len();
        Ok(())
    }

    fn migrate_tools(
        db: &Database,
        config: &AppConfig,
        result: &mut MigrationResult,
    ) -> Result<(), AppError> {
        let repo = crate::core::database::ToolsRepository::new(db);
        let existing = repo.get_all()?;
        if !existing.is_empty() {
            return Ok(());
        }

        for tool in &config.tools {
            repo.upsert(tool)?;
        }

        result.tools_migrated = config.tools.len();
        Ok(())
    }

    fn migrate_library_skills(
        db: &Database,
        config: &AppConfig,
        result: &mut MigrationResult,
    ) -> Result<(), AppError> {
        let repo = crate::core::database::SkillsRepository::new(db);
        let existing = repo.get_installed()?;
        if !existing.is_empty() {
            return Ok(());
        }

        let library_path = &config.library_path;
        if !library_path.exists() {
            return Ok(());
        }

        let mut count = 0;
        for entry in fs::read_dir(library_path)
            .map_err(|e| AppError::Config(format!("Failed to read library dir: {}", e)))?
        {
            let entry =
                entry.map_err(|e| AppError::Config(format!("Failed to read dir entry: {}", e)))?;
            if !entry
                .file_type()
                .map_err(|e| AppError::Config(e.to_string()))?
                .is_dir()
            {
                continue;
            }
            let skill_dir = entry.path();
            let skill_md = match crate::core::fs_utils::find_skill_marker(&skill_dir) {
                Some(p) => p,
                None => continue,
            };

            let name = entry.file_name().to_string_lossy().into_owned();
            let skill_id = crate::core::library::SkillLibrary::compute_skill_id(&name, &skill_dir);

            let description = fs::read_to_string(&skill_md)
                .ok()
                .and_then(|content| {
                    content
                        .lines()
                        .find(|l| l.starts_with("description:"))
                        .map(|l| l.trim_start_matches("description:").trim().to_string())
                })
                .unwrap_or_default();

            let skill = crate::core::models::Skill {
                id: skill_id,
                name: name.clone(),
                path_hash: crate::core::library::SkillLibrary::compute_path_hash(&skill_dir),
                library_path: skill_dir.to_string_lossy().into_owned(),
                original_source_path: None,
                original_git_url: None,
                original_git_subpath: None,
                group: "library".to_string(),
                description,
                frontmatter: std::collections::HashMap::new(),
                created_at: chrono::Utc::now().to_rfc3339(),
                mtime_ms: fs::metadata(&skill_dir)
                    .ok()
                    .and_then(|m| m.modified().ok())
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|d| d.as_millis() as i64)
                    .unwrap_or(0),
                source_type: crate::core::models::SkillSourceType::LocalFolder,
                is_deleted: false,
                content_hash: None,
                source_revision: None,
                source_remote_revision: None,
                source_update_status: Default::default(),
            };

            repo.upsert(&skill)?;
            repo.mark_installed(&skill.id)?;
            count += 1;
        }

        result.skills_migrated = count;
        Ok(())
    }
}

#[derive(Debug, Default)]
pub struct MigrationResult {
    pub config_migrated: bool,
    pub audit_migrated: usize,
    pub tools_migrated: usize,
    pub skills_migrated: usize,
}

impl std::fmt::Display for MigrationResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Migration complete: config={}, audit={}, tools={}, skills={}",
            self.config_migrated, self.audit_migrated, self.tools_migrated, self.skills_migrated
        )
    }
}
