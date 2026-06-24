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
        Self::migrate_sync_credentials(db)?;

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

    /// Merge old per-key credentials (from the config table) into each
    /// webdav provider's `config_json`. This bridges the v7 schema (where
    /// `webdav_username` / `webdav_password` lived in the config table as
    /// separate encrypted rows) to the v8+ model (where everything is
    /// inside the provider's `config_json`, encrypted at the field level
    /// by `SyncProviderRepository::encrypt_config`).
    ///
    /// Runs once; after the keys are removed from the config table the
    /// function becomes a no-op.
    fn migrate_sync_credentials(db: &Database) -> Result<(), AppError> {
        let config_repo = crate::core::database::ConfigRepository::new(db);
        let provider_repo = crate::core::database::SyncProviderRepository::new(db);

        // ConfigRepository::get() already decrypts sensitive keys.
        let username = config_repo.get("webdav_username").ok().flatten();
        let password = config_repo.get("webdav_password").ok().flatten();

        if username.is_none() && password.is_none() {
            return Ok(());
        }

        let plain_user = username.unwrap_or_default();
        let plain_pass = password.unwrap_or_default();

        // Merge into each webdav provider's config_json.
        let providers = provider_repo.list().ok().unwrap_or_default();
        for provider in &providers {
            if provider.kind != crate::core::models::SyncProviderKind::WebDav {
                continue;
            }
            let mut config: serde_json::Value =
                serde_json::from_str(&provider.config_json).unwrap_or(serde_json::json!({}));
            let obj = config.as_object_mut().unwrap();
            if !plain_user.is_empty() && !obj.contains_key("username") {
                obj.insert("username".into(), serde_json::Value::String(plain_user.clone()));
            }
            if !plain_pass.is_empty() && !obj.contains_key("password") {
                obj.insert("password".into(), serde_json::Value::String(plain_pass.clone()));
            }
            let new_json = serde_json::to_string(&config).unwrap_or_else(|_| provider.config_json.clone());
            // Write directly via SQL to avoid re-encryption by the repository.
            let conn = db.connection();
            let _ = conn.execute(
                "UPDATE sync_providers SET config_json = ?1 WHERE id = ?2",
                rusqlite::params![new_json, provider.id],
            );
        }

        // Remove old keys so this migration is idempotent.
        let _ = config_repo.delete("webdav_username");
        let _ = config_repo.delete("webdav_password");
        let _ = config_repo.delete("webdav_url");
        let _ = config_repo.delete("github_token");
        let _ = config_repo.delete("github_repo");
        let _ = config_repo.delete("backup_archive_password");

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
