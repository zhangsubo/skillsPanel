use crate::core::config::AppConfig;
use crate::core::error::AppError;
use crate::core::models::AuditEntry;
use std::fs;
use std::path::PathBuf;

pub struct AuditLog {
    entries: Vec<AuditEntry>,
    log_path: PathBuf,
}

impl AuditLog {
    pub fn new(_config: &AppConfig) -> Result<Self, AppError> {
        let config_dir = dirs::home_dir()
            .ok_or_else(|| AppError::Config("Cannot find home directory".into()))?
            .join(".skills-panel");
        let log_path = config_dir.join("audit.json");
        let entries = if log_path.exists() {
            let content = fs::read_to_string(&log_path)?;
            serde_json::from_str(&content).unwrap_or_default()
        } else {
            Vec::new()
        };
        Ok(Self { entries, log_path })
    }

    pub fn log(
        &mut self,
        action: &str,
        target: &str,
        details: Option<String>,
        success: bool,
        error: Option<String>,
    ) {
        let entry = AuditEntry {
            timestamp: chrono::Utc::now().to_rfc3339(),
            action: action.to_string(),
            target: target.to_string(),
            details,
            success,
            error,
        };
        self.entries.push(entry);
        self.save_to_json();
    }

    pub fn log_to_db(
        &self,
        db: &crate::core::database::Database,
        action: &str,
        target: &str,
        details: Option<String>,
        success: bool,
        error: Option<String>,
    ) -> Result<(), AppError> {
        let repo = crate::core::database::AuditRepository::new(db);
        repo.log(action, target, details, success, error)
    }

    pub fn get_logs(&self, limit: usize) -> Vec<AuditEntry> {
        self.entries.iter().rev().take(limit).cloned().collect()
    }

    fn save_to_json(&self) {
        if let Ok(json) = serde_json::to_string_pretty(&self.entries) {
            let _ = fs::write(&self.log_path, json);
        }
    }

    pub fn sync_to_database(&self, db: &crate::core::database::Database) -> Result<(), AppError> {
        let repo = crate::core::database::AuditRepository::new(db);
        for entry in &self.entries {
            repo.log(
                &entry.action,
                &entry.target,
                entry.details.clone(),
                entry.success,
                entry.error.clone(),
            )?;
        }
        Ok(())
    }
}
