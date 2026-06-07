use crate::core::crypto::Crypto;
use crate::core::error::AppError;
use crate::core::models::{
    AuditEntry, BulkAttachResult, Skill, SkillSourceType, SyncHistory, SyncProvider, Tag, Tool,
};
use rusqlite::{params, Connection, OptionalExtension};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;

/// Current schema version. Bump this when adding new migration steps.
const LATEST_VERSION: u32 = 7;

/// Max length of a tag name. Long enough for human-friendly labels
/// ("frontend / react-hooks"), short enough to keep the UNIQUE index cheap.
pub const MAX_TAG_NAME_LEN: usize = 64;
/// Max `skill_ids` accepted by `bulk_attach_tag` per call. Bounds the time the
/// DB Mutex is held and the per-call IPC payload size.
pub const BULK_TAG_ATTACH_MAX: usize = 5000;
/// Max bytes accepted by `import_archive` per call. Bounds the DB Mutex hold
/// time and IPC payload size when restoring from a cloud backup.
pub const ARCHIVE_IMPORT_MAX: usize = 2 * 1024 * 1024 * 1024; // 2 GiB

fn validate_tag_name(name: &str) -> Result<&str, AppError> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err(AppError::Config("Tag name cannot be empty".into()));
    }
    if trimmed.chars().any(|c| c.is_control()) {
        return Err(AppError::Config(
            "Tag name cannot contain control characters".into(),
        ));
    }
    if trimmed.chars().count() > MAX_TAG_NAME_LEN {
        return Err(AppError::Config(format!(
            "Tag name too long: max {MAX_TAG_NAME_LEN} characters"
        )));
    }
    Ok(trimmed)
}

pub struct Database {
    conn: Mutex<Connection>,
    crypto: Option<Crypto>,
}

impl Database {
    pub fn new(db_path: &PathBuf) -> Result<Self, AppError> {
        let conn = Connection::open(db_path)
            .map_err(|e| AppError::Config(format!("Failed to open database: {}", e)))?;

        conn.busy_timeout(std::time::Duration::from_secs(5))
            .map_err(|e| AppError::Config(format!("Failed to set busy timeout: {}", e)))?;

        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")
            .map_err(|e| AppError::Config(format!("Failed to set pragmas: {}", e)))?;

        Self::run_migrations(&conn)?;

        let key_dir = db_path.parent().unwrap_or(db_path);
        let crypto = Crypto::new(key_dir).ok();

        Ok(Self {
            conn: Mutex::new(conn),
            crypto,
        })
    }

    pub fn crypto(&self) -> Option<&Crypto> {
        self.crypto.as_ref()
    }

    pub fn connection(&self) -> std::sync::MutexGuard<'_, Connection> {
        self.conn.lock().unwrap()
    }

    // ── Schema & Migrations ──────────────────────────────────────────

    fn run_migrations(conn: &Connection) -> Result<(), AppError> {
        let current: u32 = conn
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .map_err(|e| AppError::Config(format!("Failed to read user_version: {}", e)))?;

        if current > LATEST_VERSION {
            return Err(AppError::Config(format!(
                "Database version {} is newer than supported version {}. Cannot downgrade.",
                current, LATEST_VERSION
            )));
        }

        for version in current..LATEST_VERSION {
            conn.execute_batch("BEGIN EXCLUSIVE")
                .map_err(|e| AppError::Config(format!("Failed to begin migration txn: {}", e)))?;
            Self::migrate_step(conn, version)?;
            conn.pragma_update(None, "user_version", version + 1)
                .map_err(|e| AppError::Config(format!("Failed to update version: {}", e)))?;
            conn.execute_batch("COMMIT")
                .map_err(|e| AppError::Config(format!("Failed to commit migration: {}", e)))?;
        }

        Ok(())
    }

    fn migrate_step(conn: &Connection, from_version: u32) -> Result<(), AppError> {
        match from_version {
            0 => Self::migrate_v0_to_v1(conn),
            1 => Self::migrate_v1_to_v2(conn),
            2 => Self::migrate_v2_to_v3(conn),
            3 => Self::migrate_v3_to_v4(conn),
            4 => Self::migrate_v4_to_v5(conn),
            5 => Self::migrate_v5_to_v6(conn),
            6 => Self::migrate_v6_to_v7(conn),
            _ => Err(AppError::Config(format!(
                "No migration path from version {}",
                from_version
            ))),
        }
    }

    /// v0 → v1: Initial schema with all tables including unified scan support.
    fn migrate_v0_to_v1(conn: &Connection) -> Result<(), AppError> {
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS skills (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                path_hash TEXT NOT NULL,
                library_path TEXT NOT NULL,
                original_source_path TEXT,
                original_git_url TEXT,
                original_git_subpath TEXT,
                group_name TEXT NOT NULL DEFAULT 'default',
                description TEXT NOT NULL DEFAULT '',
                frontmatter TEXT NOT NULL DEFAULT '{}',
                created_at TEXT NOT NULL,
                mtime_ms INTEGER NOT NULL DEFAULT 0,
                source_type TEXT NOT NULL DEFAULT 'local-folder',
                is_deleted INTEGER NOT NULL DEFAULT 0,
                last_seen_at TEXT NOT NULL,
                first_seen_at TEXT NOT NULL,
                is_installed INTEGER NOT NULL DEFAULT 0,
                installed_at TEXT,
                content_hash TEXT,
                sync_status TEXT NOT NULL DEFAULT 'pending'
            );

            CREATE INDEX IF NOT EXISTS idx_skills_name ON skills(name);
            CREATE INDEX IF NOT EXISTS idx_skills_installed ON skills(is_installed);
            CREATE INDEX IF NOT EXISTS idx_skills_source ON skills(original_source_path);
            CREATE INDEX IF NOT EXISTS idx_skills_last_seen ON skills(last_seen_at);
            CREATE INDEX IF NOT EXISTS idx_skills_deleted ON skills(is_deleted);

            CREATE TABLE IF NOT EXISTS tools (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                path TEXT NOT NULL,
                enabled INTEGER NOT NULL DEFAULT 1,
                is_custom INTEGER NOT NULL DEFAULT 0,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_tools_enabled ON tools(enabled);

            CREATE TABLE IF NOT EXISTS tool_skill_links (
                tool_id TEXT NOT NULL,
                skill_id TEXT NOT NULL,
                linked_at TEXT NOT NULL,
                status TEXT NOT NULL DEFAULT 'active',
                PRIMARY KEY (tool_id, skill_id),
                FOREIGN KEY (tool_id) REFERENCES tools(id) ON DELETE CASCADE,
                FOREIGN KEY (skill_id) REFERENCES skills(id) ON DELETE CASCADE
            );

            CREATE INDEX IF NOT EXISTS idx_links_tool ON tool_skill_links(tool_id);
            CREATE INDEX IF NOT EXISTS idx_links_skill ON tool_skill_links(skill_id);

            CREATE TABLE IF NOT EXISTS config (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL,
                category TEXT NOT NULL DEFAULT 'general',
                updated_at TEXT NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_config_category ON config(category);

            CREATE TABLE IF NOT EXISTS audit_log (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                timestamp TEXT NOT NULL,
                action TEXT NOT NULL,
                target TEXT NOT NULL,
                details TEXT,
                success INTEGER NOT NULL DEFAULT 1,
                error TEXT,
                user_id TEXT,
                created_at TEXT NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_audit_timestamp ON audit_log(timestamp);
            CREATE INDEX IF NOT EXISTS idx_audit_action ON audit_log(action);

            CREATE TABLE IF NOT EXISTS app_logs (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                timestamp TEXT NOT NULL,
                level TEXT NOT NULL,
                message TEXT NOT NULL,
                source TEXT NOT NULL,
                created_at TEXT NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_logs_timestamp ON app_logs(timestamp);
            CREATE INDEX IF NOT EXISTS idx_logs_level ON app_logs(level);

            CREATE TABLE IF NOT EXISTS marketplace_cache (
                cache_key TEXT PRIMARY KEY,
                data TEXT NOT NULL,
                fetched_at INTEGER NOT NULL
            );

            CREATE VIEW IF NOT EXISTS v_installed_skills AS
            SELECT
                s.*,
                GROUP_CONCAT(t.name, ', ') as linked_tools,
                COUNT(tsl.skill_id) as linked_count
            FROM skills s
            LEFT JOIN tool_skill_links tsl ON s.id = tsl.skill_id AND tsl.status = 'active'
            LEFT JOIN tools t ON tsl.tool_id = t.id
            WHERE s.is_installed = 1 AND s.is_deleted = 0
            GROUP BY s.id;

            CREATE TABLE IF NOT EXISTS projects (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                root_path TEXT NOT NULL UNIQUE,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );",
        )
        .map_err(|e| AppError::Config(format!("Failed to create initial schema: {}", e)))?;
        Ok(())
    }

    /// v1 → v2: Add content_hash column if missing (idempotent).
    fn migrate_v1_to_v2(conn: &Connection) -> Result<(), AppError> {
        Self::add_column_if_missing(conn, "skills", "content_hash", "TEXT");
        Ok(())
    }

    /// v2 → v3: Add marketplace_cache table for skills.sh search caching.
    fn migrate_v2_to_v3(conn: &Connection) -> Result<(), AppError> {
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS marketplace_cache (
                cache_key TEXT PRIMARY KEY,
                data TEXT NOT NULL,
                fetched_at INTEGER NOT NULL
            );",
        )
        .map_err(|e| {
            AppError::Config(format!("Failed to create marketplace_cache table: {}", e))
        })?;
        Ok(())
    }

    /// v3 → v4: Add projects table for workspace management.
    fn migrate_v3_to_v4(conn: &Connection) -> Result<(), AppError> {
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS projects (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                root_path TEXT NOT NULL UNIQUE,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );",
        )
        .map_err(|e| AppError::Config(format!("Failed to create projects table: {}", e)))?;
        Ok(())
    }

    /// v4 → v5: Add source revision tracking columns to skills table.
    fn migrate_v4_to_v5(conn: &Connection) -> Result<(), AppError> {
        Self::add_column_if_missing(conn, "skills", "source_revision", "TEXT");
        Self::add_column_if_missing(conn, "skills", "source_remote_revision", "TEXT");
        Self::add_column_if_missing(
            conn,
            "skills",
            "source_update_status",
            "TEXT NOT NULL DEFAULT 'up-to-date'",
        );
        Ok(())
    }

    /// v5 → v6: Add tags + skill_tags tables for user-defined skill grouping.
    /// Tags live entirely in the local DB; SKILL.md is never modified.
    fn migrate_v5_to_v6(conn: &Connection) -> Result<(), AppError> {
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS tags (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL UNIQUE,
                color TEXT,
                description TEXT,
                created_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS skill_tags (
                skill_id TEXT NOT NULL,
                tag_id TEXT NOT NULL,
                created_at TEXT NOT NULL,
                PRIMARY KEY (skill_id, tag_id),
                FOREIGN KEY (skill_id) REFERENCES skills(id) ON DELETE CASCADE,
                FOREIGN KEY (tag_id) REFERENCES tags(id) ON DELETE CASCADE
            );

            CREATE INDEX IF NOT EXISTS idx_skill_tags_skill ON skill_tags(skill_id);
            CREATE INDEX IF NOT EXISTS idx_skill_tags_tag ON skill_tags(tag_id);",
        )
        .map_err(|e| AppError::Config(format!("Failed v5→v6 migration: {}", e)))?;
        Ok(())
    }

    /// v6 → v7: Add sync_providers + sync_history tables for cloud backup.
    /// Credentials live in the config table (encrypted via SENSITIVE_KEYS);
    /// these two tables only hold provider metadata + per-attempt audit.
    /// CASCADE on provider_id means deleting a provider auto-cleans its history.
    fn migrate_v6_to_v7(conn: &Connection) -> Result<(), AppError> {
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS sync_providers (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL UNIQUE,
                kind TEXT NOT NULL,
                config_json TEXT NOT NULL,
                enabled INTEGER NOT NULL DEFAULT 1,
                last_sync_at TEXT,
                last_sync_status TEXT,
                last_sync_error TEXT,
                created_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS sync_history (
                id TEXT PRIMARY KEY,
                provider_id TEXT NOT NULL,
                direction TEXT NOT NULL,
                status TEXT NOT NULL,
                started_at TEXT NOT NULL,
                finished_at TEXT,
                bytes_transferred INTEGER,
                skills_count INTEGER,
                error_message TEXT,
                FOREIGN KEY (provider_id) REFERENCES sync_providers(id) ON DELETE CASCADE
            );

            CREATE INDEX IF NOT EXISTS idx_sync_history_provider ON sync_history(provider_id);
            CREATE INDEX IF NOT EXISTS idx_sync_history_started ON sync_history(started_at);",
        )
        .map_err(|e| AppError::Config(format!("Failed v6→v7 migration: {}", e)))?;
        Ok(())
    }

    /// Safely add a column only if it doesn't already exist.
    fn add_column_if_missing(conn: &Connection, table: &str, column: &str, col_type: &str) {
        let check = format!(
            "SELECT COUNT(*) FROM pragma_table_info('{}') WHERE name = '{}'",
            table, column
        );
        let exists: bool = conn
            .query_row(&check, [], |row| row.get::<_, i64>(0))
            .map(|v| v > 0)
            .unwrap_or(false);

        if !exists {
            let alter = format!("ALTER TABLE {} ADD COLUMN {} {}", table, column, col_type);
            let _ = conn.execute_batch(&alter);
        }
    }
}

// ── Skills Repository ────────────────────────────────────────────────

pub struct SkillsRepository<'a> {
    db: &'a Database,
}

impl<'a> SkillsRepository<'a> {
    pub fn new(db: &'a Database) -> Self {
        Self { db }
    }

    pub fn upsert(&self, skill: &Skill) -> Result<(), AppError> {
        let conn = self.db.connection();
        let frontmatter_json = serde_json::to_string(&skill.frontmatter)
            .map_err(|e| AppError::Config(format!("Failed to serialize frontmatter: {}", e)))?;

        let source_type_str = match skill.source_type {
            SkillSourceType::Git => "git",
            SkillSourceType::LocalZip => "local-zip",
            SkillSourceType::LocalFolder => "local-folder",
        };

        let update_status_str = match skill.source_update_status {
            crate::core::models::SourceUpdateStatus::UpToDate => "up-to-date",
            crate::core::models::SourceUpdateStatus::UpdateAvailable => "update-available",
            crate::core::models::SourceUpdateStatus::Unknown => "unknown",
        };

        let now = chrono::Utc::now().to_rfc3339();

        conn.execute(
            "INSERT INTO skills (
                id, name, path_hash, library_path, original_source_path,
                original_git_url, original_git_subpath, group_name, description,
                frontmatter, created_at, mtime_ms, source_type, is_deleted,
                last_seen_at, first_seen_at, is_installed, installed_at,
                source_revision, source_remote_revision, source_update_status,
                content_hash
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22)
            ON CONFLICT(id) DO UPDATE SET
                name = excluded.name,
                path_hash = excluded.path_hash,
                library_path = excluded.library_path,
                original_source_path = excluded.original_source_path,
                original_git_url = excluded.original_git_url,
                description = excluded.description,
                frontmatter = excluded.frontmatter,
                mtime_ms = excluded.mtime_ms,
                is_deleted = 0,
                last_seen_at = excluded.last_seen_at,
                is_installed = 1,
                installed_at = ?18,
                source_revision = COALESCE(excluded.source_revision, skills.source_revision),
                source_remote_revision = COALESCE(excluded.source_remote_revision, skills.source_remote_revision),
                source_update_status = COALESCE(excluded.source_update_status, skills.source_update_status),
                content_hash = excluded.content_hash",
            params![
                skill.id, skill.name, skill.path_hash, skill.library_path,
                skill.original_source_path, skill.original_git_url, skill.original_git_subpath,
                skill.group, skill.description, frontmatter_json, skill.created_at,
                skill.mtime_ms, source_type_str, skill.is_deleted as i32,
                now, skill.created_at,
                1 as i32, now,
                skill.source_revision, skill.source_remote_revision, update_status_str,
                skill.content_hash,
            ],
        )
        .map_err(|e| AppError::Config(format!("Failed to upsert skill: {}", e)))?;

        Ok(())
    }

    pub fn get_installed(&self) -> Result<Vec<Skill>, AppError> {
        let conn = self.db.connection();
        let mut stmt = conn
            .prepare(
                "SELECT id, name, path_hash, library_path, original_source_path,
                        original_git_url, original_git_subpath, group_name, description,
                        frontmatter, created_at, mtime_ms, source_type, is_deleted,
                        source_revision, source_remote_revision, source_update_status,
                        content_hash
                 FROM skills WHERE is_installed = 1 AND is_deleted = 0 ORDER BY name",
            )
            .map_err(|e| AppError::Config(format!("Failed to prepare query: {}", e)))?;

        Self::query_skills_from_stmt(&mut stmt, [])
    }

    pub fn get_all_active(&self) -> Result<Vec<Skill>, AppError> {
        let conn = self.db.connection();
        let mut stmt = conn
            .prepare(
                "SELECT id, name, path_hash, library_path, original_source_path,
                        original_git_url, original_git_subpath, group_name, description,
                        frontmatter, created_at, mtime_ms, source_type, is_deleted,
                        source_revision, source_remote_revision, source_update_status,
                        content_hash
                 FROM skills WHERE is_deleted = 0 ORDER BY name",
            )
            .map_err(|e| AppError::Config(format!("Failed to prepare query: {}", e)))?;

        Self::query_skills_from_stmt(&mut stmt, [])
    }

    pub fn mark_installed(&self, skill_id: &str) -> Result<(), AppError> {
        let conn = self.db.connection();
        let now = chrono::Utc::now().to_rfc3339();
        let rows_changed = conn
            .execute(
                "UPDATE skills SET is_installed = 1, installed_at = ?1 WHERE id = ?2",
                params![now, skill_id],
            )
            .map_err(|e| AppError::Config(format!("Failed to mark skill as installed: {}", e)))?;

        if rows_changed == 0 {
            return Err(AppError::Config(format!(
                "No skill found with id: {}",
                skill_id
            )));
        }

        Ok(())
    }

    pub fn mark_uninstalled(&self, skill_name: &str) -> Result<(), AppError> {
        let conn = self.db.connection();
        conn.execute(
            "UPDATE skills SET is_installed = 0, installed_at = NULL WHERE name = ?1",
            params![skill_name],
        )
        .map_err(|e| AppError::Config(format!("Failed to mark skill as uninstalled: {}", e)))?;
        Ok(())
    }

    pub fn delete_by_name(&self, skill_name: &str) -> Result<(), AppError> {
        let conn = self.db.connection();
        conn.execute("DELETE FROM skills WHERE name = ?1", params![skill_name])
            .map_err(|e| AppError::Config(format!("Failed to delete skill: {}", e)))?;
        Ok(())
    }

    pub fn get_skill_id_by_name(&self, name: &str) -> Result<Option<String>, AppError> {
        let conn = self.db.connection();
        let result = conn
            .query_row(
                "SELECT id FROM skills WHERE name = ?1",
                params![name],
                |row| row.get(0),
            )
            .optional()
            .map_err(|e| AppError::Config(format!("Failed to query skill: {}", e)))?;
        Ok(result)
    }

    pub fn get_by_name(&self, name: &str) -> Result<Option<Skill>, AppError> {
        let conn = self.db.connection();
        let result = conn
            .query_row(
                "SELECT id, name, path_hash, library_path, original_source_path,
                        original_git_url, original_git_subpath, group_name, description,
                        frontmatter, created_at, mtime_ms, source_type, is_deleted,
                        source_revision, source_remote_revision, source_update_status,
                        content_hash
                 FROM skills WHERE name = ?1",
                params![name],
                |row| Self::row_to_skill(row),
            )
            .optional()
            .map_err(|e| AppError::Config(format!("Failed to query skill: {}", e)))?;
        Ok(result)
    }

    pub fn update_description(&self, name: &str, description: &str) -> Result<(), AppError> {
        let conn = self.db.connection();
        conn.execute(
            "UPDATE skills SET description = ?1 WHERE name = ?2",
            params![description, name],
        )
        .map_err(|e| AppError::Config(format!("Failed to update description: {}", e)))?;
        Ok(())
    }

    pub fn update_content_hash(&self, skill_id: &str, content_hash: &str) -> Result<(), AppError> {
        let conn = self.db.connection();
        conn.execute(
            "UPDATE skills SET content_hash = ?1 WHERE id = ?2",
            params![content_hash, skill_id],
        )
        .map_err(|e| AppError::Config(format!("Failed to update content hash: {}", e)))?;
        Ok(())
    }

    pub fn update_source_revision(&self, skill_id: &str, head_sha: &str) -> Result<(), AppError> {
        let conn = self.db.connection();
        conn.execute(
            "UPDATE skills SET source_revision = ?1, source_remote_revision = ?1, source_update_status = 'up-to-date' WHERE id = ?2",
            params![head_sha, skill_id],
        )
        .map_err(|e| AppError::Config(format!("Failed to update source revision: {}", e)))?;
        Ok(())
    }

    pub fn update_source_remote_revision(
        &self,
        skill_id: &str,
        remote_revision: &str,
        update_status: &crate::core::models::SourceUpdateStatus,
    ) -> Result<(), AppError> {
        let conn = self.db.connection();
        let status_str = match update_status {
            crate::core::models::SourceUpdateStatus::UpToDate => "up-to-date",
            crate::core::models::SourceUpdateStatus::UpdateAvailable => "update-available",
            crate::core::models::SourceUpdateStatus::Unknown => "unknown",
        };
        conn.execute(
            "UPDATE skills SET source_remote_revision = ?1, source_update_status = ?2 WHERE id = ?3",
            params![remote_revision, status_str, skill_id],
        )
        .map_err(|e| AppError::Config(format!("Failed to update source remote revision: {}", e)))?;
        Ok(())
    }

    // ── Scan-related methods (merged from ScanDatabase) ───────────────

    pub fn upsert_with_scan(&self, skill: &Skill, scan_timestamp: &str) -> Result<(), AppError> {
        let conn = self.db.connection();
        let frontmatter_json = serde_json::to_string(&skill.frontmatter)
            .map_err(|e| AppError::Config(format!("Failed to serialize frontmatter: {}", e)))?;

        let source_type_str = match skill.source_type {
            SkillSourceType::Git => "git",
            SkillSourceType::LocalZip => "local-zip",
            SkillSourceType::LocalFolder => "local-folder",
        };

        conn.execute(
            "INSERT INTO skills (
                id, name, path_hash, library_path, original_source_path,
                original_git_url, original_git_subpath, group_name, description,
                frontmatter, created_at, mtime_ms, source_type, is_deleted,
                last_seen_at, first_seen_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?15)
            ON CONFLICT(id) DO UPDATE SET
                name = excluded.name,
                path_hash = excluded.path_hash,
                library_path = excluded.library_path,
                original_source_path = excluded.original_source_path,
                original_git_url = excluded.original_git_url,
                description = excluded.description,
                frontmatter = excluded.frontmatter,
                mtime_ms = excluded.mtime_ms,
                is_deleted = 0,
                last_seen_at = excluded.last_seen_at",
            params![
                skill.id,
                skill.name,
                skill.path_hash,
                skill.library_path,
                skill.original_source_path,
                skill.original_git_url,
                skill.original_git_subpath,
                skill.group,
                skill.description,
                frontmatter_json,
                skill.created_at,
                skill.mtime_ms,
                source_type_str,
                skill.is_deleted as i32,
                scan_timestamp,
            ],
        )
        .map_err(|e| AppError::Config(format!("Failed to upsert skill: {}", e)))?;

        Ok(())
    }

    pub fn mark_missing_as_deleted(&self, scan_timestamp: &str) -> Result<Vec<String>, AppError> {
        let conn = self.db.connection();
        let mut stmt = conn
            .prepare(
                "UPDATE skills SET is_deleted = 1 WHERE last_seen_at < ?1 AND is_deleted = 0
                 RETURNING id",
            )
            .map_err(|e| AppError::Config(format!("Failed to prepare delete query: {}", e)))?;

        let ids: Result<Vec<String>, _> = stmt
            .query_map([scan_timestamp], |row| row.get(0))
            .map_err(|e| AppError::Config(format!("Failed to query deleted skills: {}", e)))?
            .collect();

        ids.map_err(|e| AppError::Config(format!("Failed to collect deleted skills: {}", e)))
    }

    pub fn get_new_skills(&self, scan_timestamp: &str) -> Result<Vec<Skill>, AppError> {
        let conn = self.db.connection();
        let mut stmt = conn
            .prepare(
                "SELECT id, name, path_hash, library_path, original_source_path,
                        original_git_url, original_git_subpath, group_name, description,
                        frontmatter, created_at, mtime_ms, source_type, is_deleted,
                        source_revision, source_remote_revision, source_update_status,
                        content_hash
                 FROM skills WHERE first_seen_at = ?1 AND is_deleted = 0
                 ORDER BY name",
            )
            .map_err(|e| AppError::Config(format!("Failed to prepare new skills query: {}", e)))?;

        Self::query_skills_from_stmt(&mut stmt, [scan_timestamp])
    }

    pub fn get_updated_skills(&self, scan_timestamp: &str) -> Result<Vec<Skill>, AppError> {
        let conn = self.db.connection();
        let mut stmt = conn
            .prepare(
                "SELECT id, name, path_hash, library_path, original_source_path,
                        original_git_url, original_git_subpath, group_name, description,
                        frontmatter, created_at, mtime_ms, source_type, is_deleted,
                        source_revision, source_remote_revision, source_update_status,
                        content_hash
                 FROM skills
                 WHERE last_seen_at = ?1
                 AND first_seen_at != last_seen_at
                 AND is_deleted = 0
                 ORDER BY name",
            )
            .map_err(|e| {
                AppError::Config(format!("Failed to prepare updated skills query: {}", e))
            })?;

        Self::query_skills_from_stmt(&mut stmt, [scan_timestamp])
    }

    pub fn get_deleted_skills(&self) -> Result<Vec<Skill>, AppError> {
        let conn = self.db.connection();
        let mut stmt = conn
            .prepare(
                "SELECT id, name, path_hash, library_path, original_source_path,
                        original_git_url, original_git_subpath, group_name, description,
                        frontmatter, created_at, mtime_ms, source_type, is_deleted,
                        source_revision, source_remote_revision, source_update_status,
                        content_hash
                 FROM skills WHERE is_deleted = 1 ORDER BY last_seen_at DESC",
            )
            .map_err(|e| {
                AppError::Config(format!("Failed to prepare deleted skills query: {}", e))
            })?;

        Self::query_skills_from_stmt(&mut stmt, [])
    }

    pub fn get_by_id(&self, id: &str) -> Result<Option<Skill>, AppError> {
        let conn = self.db.connection();
        let mut stmt = conn
            .prepare(
                "SELECT id, name, path_hash, library_path, original_source_path,
                        original_git_url, original_git_subpath, group_name, description,
                        frontmatter, created_at, mtime_ms, source_type, is_deleted,
                        source_revision, source_remote_revision, source_update_status,
                        content_hash
                 FROM skills WHERE id = ?1",
            )
            .map_err(|e| AppError::Config(format!("Failed to prepare skill query: {}", e)))?;

        let mut skills = Self::query_skills_from_stmt(&mut stmt, [id])?;
        Ok(skills.pop())
    }

    // ── Helpers ───────────────────────────────────────────────────────

    fn row_to_skill(row: &rusqlite::Row) -> Result<Skill, rusqlite::Error> {
        let source_type_str: String = row.get(12)?;
        let frontmatter_str: String = row.get(9)?;
        let frontmatter: HashMap<String, serde_json::Value> =
            serde_json::from_str(&frontmatter_str).unwrap_or_default();

        let source_update_status_str: Option<String> = row.get(16)?;
        let source_update_status = match source_update_status_str.as_deref() {
            Some("update-available") => crate::core::models::SourceUpdateStatus::UpdateAvailable,
            Some("unknown") => crate::core::models::SourceUpdateStatus::Unknown,
            _ => crate::core::models::SourceUpdateStatus::UpToDate,
        };

        Ok(Skill {
            id: row.get(0)?,
            name: row.get(1)?,
            path_hash: row.get(2)?,
            library_path: row.get(3)?,
            original_source_path: row.get(4)?,
            original_git_url: row.get(5)?,
            original_git_subpath: row.get(6)?,
            group: row.get(7)?,
            description: row.get(8)?,
            frontmatter,
            created_at: row.get(10)?,
            mtime_ms: row.get(11)?,
            source_type: match source_type_str.as_str() {
                "git" => SkillSourceType::Git,
                "local-zip" => SkillSourceType::LocalZip,
                _ => SkillSourceType::LocalFolder,
            },
            is_deleted: row.get::<_, i32>(13)? != 0,
            content_hash: row.get(17)?,
            source_revision: row.get(14)?,
            source_remote_revision: row.get(15)?,
            source_update_status,
        })
    }

    fn query_skills_from_stmt<P: rusqlite::Params>(
        stmt: &mut rusqlite::Statement,
        params: P,
    ) -> Result<Vec<Skill>, AppError> {
        let rows = stmt
            .query_map(params, |row| Self::row_to_skill(row))
            .map_err(|e| AppError::Config(format!("Failed to query skills: {}", e)))?;

        let mut skills = Vec::new();
        for row in rows {
            skills.push(
                row.map_err(|e| AppError::Config(format!("Failed to parse skill row: {}", e)))?,
            );
        }
        Ok(skills)
    }
}

// ── Config Repository ────────────────────────────────────────────────

pub struct ConfigRepository<'a> {
    db: &'a Database,
}

impl<'a> ConfigRepository<'a> {
    pub fn new(db: &'a Database) -> Self {
        Self { db }
    }

    pub fn get(&self, key: &str) -> Result<Option<String>, AppError> {
        let conn = self.db.connection();
        let result: Option<String> = conn
            .query_row(
                "SELECT value FROM config WHERE key = ?1",
                params![key],
                |row| row.get(0),
            )
            .optional()
            .map_err(|e| AppError::Config(format!("Failed to get config: {}", e)))?;

        match result {
            Some(val) if crate::core::crypto::is_sensitive_key(key) => {
                if let Some(crypto) = self.db.crypto() {
                    let decrypted = crypto.decrypt(&val)?;
                    Ok(Some(decrypted))
                } else {
                    Ok(Some(val))
                }
            }
            other => Ok(other),
        }
    }

    pub fn set(&self, key: &str, value: &str) -> Result<(), AppError> {
        let conn = self.db.connection();
        let now = chrono::Utc::now().to_rfc3339();

        let stored_value = if crate::core::crypto::is_sensitive_key(key) {
            if let Some(crypto) = self.db.crypto() {
                if !crate::core::crypto::Crypto::is_encrypted(value) {
                    crypto.encrypt(value)?
                } else {
                    value.to_string()
                }
            } else {
                value.to_string()
            }
        } else {
            value.to_string()
        };

        conn.execute(
            "INSERT INTO config (key, value, updated_at) VALUES (?1, ?2, ?3)
             ON CONFLICT(key) DO UPDATE SET value = ?2, updated_at = ?3",
            params![key, stored_value, now],
        )
        .map_err(|e| AppError::Config(format!("Failed to set config: {}", e)))?;
        Ok(())
    }

    pub fn get_all(&self) -> Result<HashMap<String, String>, AppError> {
        let conn = self.db.connection();
        let mut stmt = conn
            .prepare("SELECT key, value FROM config")
            .map_err(|e| AppError::Config(format!("Failed to prepare query: {}", e)))?;

        let config = stmt
            .query_map([], |row| {
                let key: String = row.get(0)?;
                let value: String = row.get(1)?;
                Ok((key, value))
            })
            .map_err(|e| AppError::Config(format!("Failed to query config: {}", e)))?
            .collect::<Result<HashMap<_, _>, _>>()
            .map_err(|e| AppError::Config(format!("Failed to collect config: {}", e)))?;

        let crypto = self.db.crypto();
        let mut result = HashMap::new();
        for (key, value) in config {
            if crate::core::crypto::is_sensitive_key(&key) {
                if let Some(c) = crypto {
                    result.insert(key, c.decrypt(&value)?);
                } else {
                    result.insert(key, value);
                }
            } else {
                result.insert(key, value);
            }
        }

        Ok(result)
    }
}

// ── Audit Repository ─────────────────────────────────────────────────

pub struct AuditRepository<'a> {
    db: &'a Database,
}

impl<'a> AuditRepository<'a> {
    pub fn new(db: &'a Database) -> Self {
        Self { db }
    }

    pub fn log(
        &self,
        action: &str,
        target: &str,
        details: Option<String>,
        success: bool,
        error: Option<String>,
    ) -> Result<(), AppError> {
        let conn = self.db.connection();
        let now = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO audit_log (timestamp, action, target, details, success, error, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![now, action, target, details, success as i32, error, now],
        )
        .map_err(|e| AppError::Config(format!("Failed to log audit entry: {}", e)))?;
        Ok(())
    }

    pub fn get_logs(&self, limit: usize) -> Result<Vec<AuditEntry>, AppError> {
        let conn = self.db.connection();
        let mut stmt = conn
            .prepare(
                "SELECT timestamp, action, target, details, success, error
                 FROM audit_log ORDER BY timestamp DESC LIMIT ?1",
            )
            .map_err(|e| AppError::Config(format!("Failed to prepare query: {}", e)))?;

        let entries = stmt
            .query_map([limit as i64], |row| {
                Ok(AuditEntry {
                    timestamp: row.get(0)?,
                    action: row.get(1)?,
                    target: row.get(2)?,
                    details: row.get(3)?,
                    success: row.get::<_, i32>(4)? != 0,
                    error: row.get(5)?,
                })
            })
            .map_err(|e| AppError::Config(format!("Failed to query audit logs: {}", e)))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| AppError::Config(format!("Failed to collect audit logs: {}", e)))?;

        Ok(entries)
    }
}

// ── App Logs Repository ──────────────────────────────────────────────

pub struct AppLogsRepository<'a> {
    db: &'a Database,
}

impl<'a> AppLogsRepository<'a> {
    pub fn new(db: &'a Database) -> Self {
        Self { db }
    }

    pub fn log(&self, level: &str, message: &str, source: &str) -> Result<(), AppError> {
        let conn = self.db.connection();
        let now = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO app_logs (timestamp, level, message, source, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![now, level, message, source, now],
        )
        .map_err(|e| AppError::Config(format!("Failed to log app message: {}", e)))?;
        Ok(())
    }

    pub fn get_logs(&self, limit: usize) -> Result<Vec<crate::core::models::LogEntry>, AppError> {
        let conn = self.db.connection();
        let mut stmt = conn
            .prepare(
                "SELECT timestamp, level, message, source
                 FROM app_logs ORDER BY timestamp DESC LIMIT ?1",
            )
            .map_err(|e| AppError::Config(format!("Failed to prepare query: {}", e)))?;

        let entries = stmt
            .query_map([limit as i64], |row| {
                Ok(crate::core::models::LogEntry {
                    timestamp: row.get(0)?,
                    level: row.get(1)?,
                    message: row.get(2)?,
                    source: row.get(3)?,
                })
            })
            .map_err(|e| AppError::Config(format!("Failed to query app logs: {}", e)))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| AppError::Config(format!("Failed to collect app logs: {}", e)))?;

        Ok(entries)
    }
}

// ── Tools Repository ─────────────────────────────────────────────────

pub struct ToolsRepository<'a> {
    db: &'a Database,
}

impl<'a> ToolsRepository<'a> {
    pub fn new(db: &'a Database) -> Self {
        Self { db }
    }

    pub fn upsert(&self, tool: &Tool) -> Result<(), AppError> {
        let conn = self.db.connection();
        let now = chrono::Utc::now().to_rfc3339();

        conn.execute(
            "INSERT INTO tools (id, name, path, enabled, is_custom, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(id) DO UPDATE SET
                name = excluded.name,
                path = excluded.path,
                enabled = excluded.enabled,
                is_custom = excluded.is_custom,
                updated_at = excluded.updated_at",
            params![
                tool.id,
                tool.name,
                tool.path,
                tool.enabled as i32,
                tool.is_custom as i32,
                now,
                now,
            ],
        )
        .map_err(|e| AppError::Config(format!("Failed to upsert tool: {}", e)))?;

        Ok(())
    }

    pub fn get_all(&self) -> Result<Vec<Tool>, AppError> {
        let conn = self.db.connection();
        let mut stmt = conn
            .prepare("SELECT id, name, path, enabled, is_custom FROM tools ORDER BY name")
            .map_err(|e| AppError::Config(format!("Failed to prepare query: {}", e)))?;

        let tools = stmt
            .query_map([], |row| {
                Ok(Tool {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    path: row.get(2)?,
                    enabled: row.get::<_, i32>(3)? != 0,
                    is_custom: row.get::<_, i32>(4)? != 0,
                })
            })
            .map_err(|e| AppError::Config(format!("Failed to query tools: {}", e)))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| AppError::Config(format!("Failed to collect tools: {}", e)))?;

        Ok(tools)
    }
}

// ── Tool-Skill Links Repository ──────────────────────────────────────

pub struct LinksRepository<'a> {
    db: &'a Database,
}

impl<'a> LinksRepository<'a> {
    pub fn new(db: &'a Database) -> Self {
        Self { db }
    }

    pub fn link(&self, tool_id: &str, skill_id: &str) -> Result<(), AppError> {
        let conn = self.db.connection();
        let now = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO tool_skill_links (tool_id, skill_id, linked_at, status)
             VALUES (?1, ?2, ?3, 'active')
             ON CONFLICT(tool_id, skill_id) DO UPDATE SET
                linked_at = excluded.linked_at,
                status = 'active'",
            params![tool_id, skill_id, now],
        )
        .map_err(|e| AppError::Config(format!("Failed to link tool-skill: {}", e)))?;
        Ok(())
    }

    pub fn unlink(&self, tool_id: &str, skill_id: &str) -> Result<(), AppError> {
        let conn = self.db.connection();
        conn.execute(
            "DELETE FROM tool_skill_links WHERE tool_id = ?1 AND skill_id = ?2",
            params![tool_id, skill_id],
        )
        .map_err(|e| AppError::Config(format!("Failed to unlink tool-skill: {}", e)))?;
        Ok(())
    }

    pub fn get_linked_tool_ids(&self, skill_id: &str) -> Result<Vec<String>, AppError> {
        let conn = self.db.connection();
        let mut stmt = conn
            .prepare(
                "SELECT tool_id FROM tool_skill_links WHERE skill_id = ?1 AND status = 'active'",
            )
            .map_err(|e| AppError::Config(format!("Failed to prepare query: {}", e)))?;

        let tool_ids = stmt
            .query_map(params![skill_id], |row| row.get(0))
            .map_err(|e| AppError::Config(format!("Failed to query linked tools: {}", e)))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| AppError::Config(format!("Failed to collect linked tools: {}", e)))?;

        Ok(tool_ids)
    }

    /// Returns `true` when an `active` link between the given tool and skill
    /// exists. Used by `link_skill` to detect a DB/FS mismatch (DB says linked
    /// but the symlink is missing on disk) and trigger a self-heal rebuild.
    pub fn is_active(&self, tool_id: &str, skill_id: &str) -> Result<bool, AppError> {
        let conn = self.db.connection();
        conn.query_row(
            "SELECT 1 FROM tool_skill_links
             WHERE tool_id = ?1 AND skill_id = ?2 AND status = 'active'
             LIMIT 1",
            params![tool_id, skill_id],
            |row| row.get::<_, i64>(0),
        )
        .optional()
        .map_err(|e| AppError::Config(format!("Failed to check active link: {}", e)))
        .map(|opt| opt.is_some())
    }
}

// ── Marketplace Cache Repository ─────────────────────────────────────

const MARKETPLACE_TTL_SECS: i64 = 120;
const MARKETPLACE_MAX_ENTRIES: usize = 150;

pub struct MarketplaceCacheRepository<'a> {
    db: &'a Database,
}

impl<'a> MarketplaceCacheRepository<'a> {
    pub fn new(db: &'a Database) -> Self {
        Self { db }
    }

    pub fn get(&self, cache_key: &str) -> Result<Option<String>, AppError> {
        let conn = self.db.connection();
        let now = chrono::Utc::now().timestamp();
        let cutoff = now - MARKETPLACE_TTL_SECS;

        let result = conn
            .query_row(
                "SELECT data FROM marketplace_cache WHERE cache_key = ?1 AND fetched_at >= ?2",
                params![cache_key, cutoff],
                |row| row.get(0),
            )
            .optional()
            .map_err(|e| AppError::Config(format!("Failed to get cache: {}", e)))?;

        Ok(result)
    }

    pub fn set(&self, cache_key: &str, data: &str) -> Result<(), AppError> {
        let conn = self.db.connection();
        let now = chrono::Utc::now().timestamp();

        conn.execute(
            "INSERT INTO marketplace_cache (cache_key, data, fetched_at) VALUES (?1, ?2, ?3)
             ON CONFLICT(cache_key) DO UPDATE SET data = ?2, fetched_at = ?3",
            params![cache_key, data, now],
        )
        .map_err(|e| AppError::Config(format!("Failed to set cache: {}", e)))?;

        Self::evict_old_entries(&conn)?;

        Ok(())
    }

    pub fn clear(&self) -> Result<(), AppError> {
        let conn = self.db.connection();
        conn.execute("DELETE FROM marketplace_cache", [])
            .map_err(|e| AppError::Config(format!("Failed to clear cache: {}", e)))?;
        Ok(())
    }

    fn evict_old_entries(conn: &Connection) -> Result<(), AppError> {
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM marketplace_cache", [], |row| {
                row.get(0)
            })
            .map_err(|e| AppError::Config(format!("Failed to count cache: {}", e)))?;

        if count as usize > MARKETPLACE_MAX_ENTRIES {
            let to_delete = count as usize - MARKETPLACE_MAX_ENTRIES;
            conn.execute(
                "DELETE FROM marketplace_cache WHERE cache_key IN (
                    SELECT cache_key FROM marketplace_cache ORDER BY fetched_at ASC LIMIT ?1
                )",
                params![to_delete as i64],
            )
            .map_err(|e| AppError::Config(format!("Failed to evict cache: {}", e)))?;
        }

        Ok(())
    }
}

// ── Tags Repository ─────────────────────────────────────────────────

pub struct TagsRepository<'a> {
    db: &'a Database,
}

impl<'a> TagsRepository<'a> {
    pub fn new(db: &'a Database) -> Self {
        Self { db }
    }

    /// Create a new tag. Returns the persisted `Tag` with generated id + timestamp.
    /// Errors if `name` collides with an existing tag (UNIQUE constraint),
    /// or if `name` is empty / too long / contains control characters.
    pub fn create(
        &self,
        name: &str,
        color: Option<&str>,
        description: Option<&str>,
    ) -> Result<Tag, AppError> {
        let name = validate_tag_name(name)?;
        let conn = self.db.connection();
        let id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO tags (id, name, color, description, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![id, name, color, description, now],
        )
        .map_err(|e| AppError::Config(format!("Failed to create tag: {}", e)))?;
        Ok(Tag {
            id,
            name: name.to_string(),
            color: color.map(str::to_string),
            description: description.map(str::to_string),
            created_at: now,
        })
    }

    pub fn get_by_id(&self, id: &str) -> Result<Option<Tag>, AppError> {
        let conn = self.db.connection();
        conn.query_row(
            "SELECT id, name, color, description, created_at
             FROM tags WHERE id = ?1",
            params![id],
            Self::row_to_tag,
        )
        .optional()
        .map_err(|e| AppError::Config(format!("Failed to get tag: {}", e)))
    }

    pub fn get_by_name(&self, name: &str) -> Result<Option<Tag>, AppError> {
        let conn = self.db.connection();
        conn.query_row(
            "SELECT id, name, color, description, created_at
             FROM tags WHERE name = ?1",
            params![name],
            Self::row_to_tag,
        )
        .optional()
        .map_err(|e| AppError::Config(format!("Failed to get tag by name: {}", e)))
    }

    /// All tags, sorted by name (stable order for the Library filter dropdown).
    pub fn list(&self) -> Result<Vec<Tag>, AppError> {
        let conn = self.db.connection();
        let mut stmt = conn
            .prepare(
                "SELECT id, name, color, description, created_at
                 FROM tags ORDER BY name",
            )
            .map_err(|e| AppError::Config(format!("Failed to list tags: {}", e)))?;
        let tags = stmt
            .query_map([], Self::row_to_tag)
            .map_err(|e| AppError::Config(format!("Failed to query tags: {}", e)))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| AppError::Config(format!("Failed to collect tags: {}", e)))?;
        Ok(tags)
    }

    /// Update mutable fields.
    ///
    /// Convention:
    /// - `None` → leave the column unchanged
    /// - `Some("")` → for `color` / `description` (nullable): clear to NULL.
    ///                 for `name` (NOT NULL): rejected by SQLite with
    ///                 "NOT NULL constraint failed" — callers must supply a
    ///                 non-empty name (use a placeholder like `"untitled"` if
    ///                 you really need to blank the visible label).
    /// - `Some(non_empty)` → set the column to that value
    ///
    /// Why: a single `Option<&str>` would conflate "don't touch" with "clear";
    /// we trade a small string-only restriction for a much simpler call site
    /// than `Option<Option<&str>>`.
    pub fn update(
        &self,
        id: &str,
        name: Option<&str>,
        color: Option<&str>,
        description: Option<&str>,
    ) -> Result<(), AppError> {
        let conn = self.db.connection();

        // Treat `Some("")` as "clear" so the same Option<&str> can express
        // leave-unchanged / clear / set-value across all three columns.
        // Pattern: outer `if let Some(...)` filters out the no-op, then we
        // convert the empty string to None for the SQL bind.
        if let Some(n) = name {
            let value: Option<&str> = if n.is_empty() { None } else { Some(n) };
            conn.execute(
                "UPDATE tags SET name = ?1 WHERE id = ?2",
                params![value, id],
            )
            .map_err(|e| AppError::Config(format!("Failed to update tag name: {}", e)))?;
        }
        if let Some(c) = color {
            let value: Option<&str> = if c.is_empty() { None } else { Some(c) };
            conn.execute(
                "UPDATE tags SET color = ?1 WHERE id = ?2",
                params![value, id],
            )
            .map_err(|e| AppError::Config(format!("Failed to update tag color: {}", e)))?;
        }
        if let Some(d) = description {
            let value: Option<&str> = if d.is_empty() { None } else { Some(d) };
            conn.execute(
                "UPDATE tags SET description = ?1 WHERE id = ?2",
                params![value, id],
            )
            .map_err(|e| AppError::Config(format!("Failed to update tag description: {}", e)))?;
        }
        Ok(())
    }

    /// Delete a tag. `skill_tags` rows are removed by the FK CASCADE.
    pub fn delete(&self, id: &str) -> Result<(), AppError> {
        let conn = self.db.connection();
        conn.execute("DELETE FROM tags WHERE id = ?1", params![id])
            .map_err(|e| AppError::Config(format!("Failed to delete tag: {}", e)))?;
        Ok(())
    }

    /// Attach a tag to a skill. Idempotent: re-attaching is a no-op.
    pub fn attach(&self, skill_id: &str, tag_id: &str) -> Result<(), AppError> {
        let conn = self.db.connection();
        let now = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "INSERT OR IGNORE INTO skill_tags (skill_id, tag_id, created_at)
             VALUES (?1, ?2, ?3)",
            params![skill_id, tag_id, now],
        )
        .map_err(|e| AppError::Config(format!("Failed to attach tag: {}", e)))?;
        Ok(())
    }

    pub fn detach(&self, skill_id: &str, tag_id: &str) -> Result<(), AppError> {
        let conn = self.db.connection();
        conn.execute(
            "DELETE FROM skill_tags WHERE skill_id = ?1 AND tag_id = ?2",
            params![skill_id, tag_id],
        )
        .map_err(|e| AppError::Config(format!("Failed to detach tag: {}", e)))?;
        Ok(())
    }

    /// Attach the same tag to many skills in one transaction.
    ///
    /// Per-row outcomes are reported back so callers can show "applied to N
    /// of M" instead of a silent best-effort:
    /// - `attached`: rows newly inserted (re-attaching is a no-op, not counted)
    /// - `skipped`: skill_ids that don't exist in `skills` (FK violation)
    ///
    /// Any other DB error propagates as `Err`.
    pub fn bulk_attach(
        &self,
        skill_ids: &[&str],
        tag_id: &str,
    ) -> Result<BulkAttachResult, AppError> {
        let conn = self.db.connection();
        let now = chrono::Utc::now().to_rfc3339();
        let mut stmt = conn
            .prepare(
                "INSERT OR IGNORE INTO skill_tags (skill_id, tag_id, created_at)
                 VALUES (?1, ?2, ?3)",
            )
            .map_err(|e| AppError::Config(format!("Failed to prepare bulk attach: {}", e)))?;
        let mut attached: usize = 0;
        let mut skipped: usize = 0;
        for skill_id in skill_ids {
            match stmt.execute(params![skill_id, tag_id, now]) {
                Ok(changes) => attached += changes as usize,
                // SQLITE_CONSTRAINT_FOREIGNKEY = 787 (extended code).
                // The primary `err.code` is generic ConstraintViolation in
                // rusqlite 0.31, so we read the raw extended_code directly.
                Err(rusqlite::Error::SqliteFailure(err, _)) if err.extended_code == 787 => {
                    skipped += 1;
                }
                Err(e) => {
                    return Err(AppError::Config(format!(
                        "bulk_attach failed for skill_id={}: {}",
                        skill_id, e
                    )))
                }
            }
        }
        Ok(BulkAttachResult { attached, skipped })
    }

    /// All tags currently attached to a skill.
    pub fn list_tags_for_skill(&self, skill_id: &str) -> Result<Vec<Tag>, AppError> {
        let conn = self.db.connection();
        let mut stmt = conn
            .prepare(
                "SELECT t.id, t.name, t.color, t.description, t.created_at
                 FROM tags t
                 JOIN skill_tags st ON st.tag_id = t.id
                 WHERE st.skill_id = ?1
                 ORDER BY t.name",
            )
            .map_err(|e| AppError::Config(format!("Failed to list tags for skill: {}", e)))?;
        let tags = stmt
            .query_map(params![skill_id], Self::row_to_tag)
            .map_err(|e| AppError::Config(format!("Failed to query skill tags: {}", e)))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| AppError::Config(format!("Failed to collect skill tags: {}", e)))?;
        Ok(tags)
    }

    /// Skill ids that have a given tag attached.
    pub fn list_skills_for_tag(&self, tag_id: &str) -> Result<Vec<String>, AppError> {
        let conn = self.db.connection();
        let mut stmt = conn
            .prepare("SELECT skill_id FROM skill_tags WHERE tag_id = ?1 ORDER BY skill_id")
            .map_err(|e| AppError::Config(format!("Failed to prepare skills-for-tag: {}", e)))?;
        let ids = stmt
            .query_map(params![tag_id], |row| row.get::<_, String>(0))
            .map_err(|e| AppError::Config(format!("Failed to query skills-for-tag: {}", e)))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| AppError::Config(format!("Failed to collect skills-for-tag: {}", e)))?;
        Ok(ids)
    }

    /// Bulk variant for the Library page: returns one entry per (skill_id, tag)
    /// link, ready for the UI to group into `Map<skill_id, Vec<Tag>>`.
    /// Skills with no tags are absent from the result — callers should treat
    /// missing entries as "no tags".
    pub fn list_all_skill_tags(&self) -> Result<Vec<(String, Tag)>, AppError> {
        let conn = self.db.connection();
        let mut stmt = conn
            .prepare(
                "SELECT st.skill_id, t.id, t.name, t.color, t.description, t.created_at
                 FROM skill_tags st
                 JOIN tags t ON t.id = st.tag_id
                 ORDER BY st.skill_id, t.name",
            )
            .map_err(|e| AppError::Config(format!("Failed to prepare all-skill-tags: {}", e)))?;
        let rows = stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    Tag {
                        id: row.get(1)?,
                        name: row.get(2)?,
                        color: row.get(3)?,
                        description: row.get(4)?,
                        created_at: row.get(5)?,
                    },
                ))
            })
            .map_err(|e| AppError::Config(format!("Failed to query all-skill-tags: {}", e)))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| AppError::Config(format!("Failed to collect all-skill-tags: {}", e)))?;
        Ok(rows)
    }

    fn row_to_tag(row: &rusqlite::Row<'_>) -> rusqlite::Result<Tag> {
        Ok(Tag {
            id: row.get(0)?,
            name: row.get(1)?,
            color: row.get(2)?,
            description: row.get(3)?,
            created_at: row.get(4)?,
        })
    }
}

// ── Projects Repository ─────────────────────────────────────────────

pub struct ProjectsRepository<'a> {
    db: &'a Database,
}

impl<'a> ProjectsRepository<'a> {
    pub fn new(db: &'a Database) -> Self {
        Self { db }
    }

    pub fn create(&self, id: &str, name: &str, root_path: &str) -> Result<(), AppError> {
        let conn = self.db.connection();
        let now = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO projects (id, name, root_path, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![id, name, root_path, now, now],
        )
        .map_err(|e| AppError::Config(format!("Failed to create project: {}", e)))?;
        Ok(())
    }

    pub fn get_all(&self) -> Result<Vec<crate::core::models::Project>, AppError> {
        let conn = self.db.connection();
        let mut stmt = conn
            .prepare(
                "SELECT id, name, root_path, created_at, updated_at FROM projects ORDER BY name",
            )
            .map_err(|e| AppError::Config(format!("Failed to prepare query: {}", e)))?;

        let projects = stmt
            .query_map([], |row| {
                Ok(crate::core::models::Project {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    root_path: row.get(2)?,
                    created_at: row.get(3)?,
                    updated_at: row.get(4)?,
                })
            })
            .map_err(|e| AppError::Config(format!("Failed to query projects: {}", e)))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| AppError::Config(format!("Failed to collect projects: {}", e)))?;

        Ok(projects)
    }

    pub fn get_by_id(&self, id: &str) -> Result<Option<crate::core::models::Project>, AppError> {
        let conn = self.db.connection();
        let result = conn
            .query_row(
                "SELECT id, name, root_path, created_at, updated_at FROM projects WHERE id = ?1",
                params![id],
                |row| {
                    Ok(crate::core::models::Project {
                        id: row.get(0)?,
                        name: row.get(1)?,
                        root_path: row.get(2)?,
                        created_at: row.get(3)?,
                        updated_at: row.get(4)?,
                    })
                },
            )
            .optional()
            .map_err(|e| AppError::Config(format!("Failed to get project: {}", e)))?;
        Ok(result)
    }

    pub fn delete(&self, id: &str) -> Result<(), AppError> {
        let conn = self.db.connection();
        conn.execute("DELETE FROM projects WHERE id = ?1", params![id])
            .map_err(|e| AppError::Config(format!("Failed to delete project: {}", e)))?;
        Ok(())
    }

    pub fn update_name(&self, id: &str, name: &str) -> Result<(), AppError> {
        let conn = self.db.connection();
        let now = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "UPDATE projects SET name = ?1, updated_at = ?2 WHERE id = ?3",
            params![name, now, id],
        )
        .map_err(|e| AppError::Config(format!("Failed to update project: {}", e)))?;
        Ok(())
    }
}

// ── Sync Providers Repository ──────────────────────────────────────
// User-defined cloud backup targets. Credentials live in the config table
// (encrypted via SENSITIVE_KEYS) — this table only stores non-secret metadata.
// Deleting a provider cascades to its sync_history (FK ON DELETE CASCADE).

/// Max length of a provider's display name. Long enough for "personal-github"
/// or "company-nextcloud", short enough to keep the UNIQUE index cheap.
pub const MAX_PROVIDER_NAME_LEN: usize = 64;

pub struct SyncProvidersRepository<'a> {
    db: &'a Database,
}

impl<'a> SyncProvidersRepository<'a> {
    pub fn new(db: &'a Database) -> Self {
        Self { db }
    }

    /// Insert a new provider. Name must be unique.
    pub fn create(
        &self,
        name: &str,
        kind: &str,
        config_json: &str,
        enabled: bool,
    ) -> Result<SyncProvider, AppError> {
        let trimmed = name.trim();
        if trimmed.is_empty() {
            return Err(AppError::Config("Provider name cannot be empty".into()));
        }
        if trimmed.chars().any(|c| c.is_control()) {
            return Err(AppError::Config(
                "Provider name cannot contain control characters".into(),
            ));
        }
        if trimmed.chars().count() > MAX_PROVIDER_NAME_LEN {
            return Err(AppError::Config(format!(
                "Provider name too long: max {MAX_PROVIDER_NAME_LEN} characters"
            )));
        }
        if config_json.is_empty() {
            return Err(AppError::Config(
                "Provider config_json cannot be empty".into(),
            ));
        }

        let id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();
        let conn = self.db.connection();
        conn.execute(
            "INSERT INTO sync_providers (id, name, kind, config_json, enabled, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![id, trimmed, kind, config_json, enabled as i32, now],
        )
        .map_err(|e| {
            if let rusqlite::Error::SqliteFailure(err, _msg) = &e {
                if err.code == rusqlite::ErrorCode::ConstraintViolation {
                    return AppError::Config(format!(
                        "Provider name '{trimmed}' already exists"
                    ));
                }
            }
            AppError::Config(format!("Failed to create provider: {}", e))
        })?;
        Ok(SyncProvider {
            id,
            name: trimmed.to_string(),
            kind: kind.to_string(),
            config_json: config_json.to_string(),
            enabled,
            last_sync_at: None,
            last_sync_status: None,
            last_sync_error: None,
            created_at: now,
        })
    }

    pub fn get(&self, id: &str) -> Result<Option<SyncProvider>, AppError> {
        let conn = self.db.connection();
        let mut stmt = conn
            .prepare_cached(
                "SELECT id, name, kind, config_json, enabled, last_sync_at,
                        last_sync_status, last_sync_error, created_at
                 FROM sync_providers WHERE id = ?1",
            )
            .map_err(|e| AppError::Config(format!("Failed to prepare get provider: {}", e)))?;
        let mut rows = stmt
            .query(params![id])
            .map_err(|e| AppError::Config(format!("Failed to query provider: {}", e)))?;
        match rows.next() {
            Ok(Some(row)) => Ok(Some(row_to_provider(row)?)),
            Ok(None) => Ok(None),
            Err(e) => Err(AppError::Config(format!("Failed to read provider row: {}", e))),
        }
    }

    pub fn get_by_name(&self, name: &str) -> Result<Option<SyncProvider>, AppError> {
        let conn = self.db.connection();
        let mut stmt = conn
            .prepare_cached(
                "SELECT id, name, kind, config_json, enabled, last_sync_at,
                        last_sync_status, last_sync_error, created_at
                 FROM sync_providers WHERE name = ?1",
            )
            .map_err(|e| AppError::Config(format!("Failed to prepare get_by_name: {}", e)))?;
        let mut rows = stmt
            .query(params![name])
            .map_err(|e| AppError::Config(format!("Failed to query provider: {}", e)))?;
        match rows.next() {
            Ok(Some(row)) => Ok(Some(row_to_provider(row)?)),
            Ok(None) => Ok(None),
            Err(e) => Err(AppError::Config(format!("Failed to read provider row: {}", e))),
        }
    }

    /// List all providers, sorted by name. `enabled` filter is optional.
    pub fn list(&self, enabled_only: bool) -> Result<Vec<SyncProvider>, AppError> {
        let conn = self.db.connection();
        let (sql, use_filter): (&str, bool) = if enabled_only {
            (
                "SELECT id, name, kind, config_json, enabled, last_sync_at,
                        last_sync_status, last_sync_error, created_at
                 FROM sync_providers WHERE enabled = 1 ORDER BY name",
                true,
            )
        } else {
            (
                "SELECT id, name, kind, config_json, enabled, last_sync_at,
                        last_sync_status, last_sync_error, created_at
                 FROM sync_providers ORDER BY name",
                false,
            )
        };
        let mut stmt = conn
            .prepare(sql)
            .map_err(|e| AppError::Config(format!("Failed to prepare list providers: {}", e)))?;
        let mapper = |row: &rusqlite::Row<'_>| -> rusqlite::Result<SyncProvider> {
            row_to_provider(row)
        };
        let providers: Vec<SyncProvider> = if use_filter {
            stmt.query_map([], mapper)?
                .collect::<rusqlite::Result<Vec<_>>>()
                .map_err(|e| AppError::Config(format!("Failed to read providers: {}", e)))?
        } else {
            stmt.query_map([], mapper)?
                .collect::<rusqlite::Result<Vec<_>>>()
                .map_err(|e| AppError::Config(format!("Failed to read providers: {}", e)))?
        };
        Ok(providers)
    }

    /// Update mutable fields. None leaves a field unchanged, Some("") clears
    /// the nullable column. `name` is non-null so it follows the leave/change
    /// convention (Some("") would surface as NOT NULL violation).
    pub fn update(
        &self,
        id: &str,
        name: Option<&str>,
        config_json: Option<&str>,
        enabled: Option<bool>,
    ) -> Result<(), AppError> {
        // Read-then-merge to keep the leave/clear/set semantics explicit.
        let current = self
            .get(id)?
            .ok_or_else(|| AppError::Config(format!("Provider {id} not found")))?;

        let new_name = match name {
            Some(n) => {
                let t = n.trim();
                if t.is_empty() {
                    return Err(AppError::Config("Provider name cannot be empty".into()));
                }
                if t.chars().any(|c| c.is_control()) {
                    return Err(AppError::Config(
                        "Provider name cannot contain control characters".into(),
                    ));
                }
                if t.chars().count() > MAX_PROVIDER_NAME_LEN {
                    return Err(AppError::Config(format!(
                        "Provider name too long: max {MAX_PROVIDER_NAME_LEN} characters"
                    )));
                }
                t.to_string()
            }
            None => current.name,
        };
        let new_config = match config_json {
            Some(c) => {
                if c.is_empty() {
                    return Err(AppError::Config(
                        "Provider config_json cannot be empty".into(),
                    ));
                }
                c.to_string()
            }
            None => current.config_json,
        };
        let new_enabled = enabled.unwrap_or(current.enabled);

        let conn = self.db.connection();
        conn.execute(
            "UPDATE sync_providers SET name = ?1, config_json = ?2, enabled = ?3 WHERE id = ?4",
            params![new_name, new_config, new_enabled as i32, id],
        )
        .map_err(|e| {
            if let rusqlite::Error::SqliteFailure(err, _msg) = &e {
                if err.code == rusqlite::ErrorCode::ConstraintViolation {
                    return AppError::Config(format!(
                        "Provider name '{new_name}' already exists"
                    ));
                }
            }
            AppError::Config(format!("Failed to update provider: {}", e))
        })?;
        Ok(())
    }

    /// Update last-sync fields. Called by SyncEngine after each attempt.
    pub fn record_last_sync(
        &self,
        id: &str,
        status: &str,
        error: Option<&str>,
    ) -> Result<(), AppError> {
        let now = chrono::Utc::now().to_rfc3339();
        let conn = self.db.connection();
        conn.execute(
            "UPDATE sync_providers SET last_sync_at = ?1, last_sync_status = ?2, last_sync_error = ?3 WHERE id = ?4",
            params![now, status, error, id],
        )
        .map_err(|e| AppError::Config(format!("Failed to record last sync: {}", e)))?;
        Ok(())
    }

    /// Physical delete. Cascades to sync_history via FK ON DELETE CASCADE.
    /// No-op on missing id (caller decides whether to error on the missing case).
    pub fn delete(&self, id: &str) -> Result<(), AppError> {
        let conn = self.db.connection();
        let affected = conn
            .execute("DELETE FROM sync_providers WHERE id = ?1", params![id])
            .map_err(|e| AppError::Config(format!("Failed to delete provider: {}", e)))?;
        if affected == 0 {
            return Err(AppError::Config(format!("Provider {id} not found")));
        }
        Ok(())
    }
}

fn row_to_provider(row: &rusqlite::Row<'_>) -> rusqlite::Result<SyncProvider> {
    Ok(SyncProvider {
        id: row.get(0)?,
        name: row.get(1)?,
        kind: row.get(2)?,
        config_json: row.get(3)?,
        enabled: row.get::<_, i32>(4)? != 0,
        last_sync_at: row.get(5)?,
        last_sync_status: row.get(6)?,
        last_sync_error: row.get(7)?,
        created_at: row.get(8)?,
    })
}

// ── Sync History Repository ────────────────────────────────────────
// Append-only audit of each sync attempt. Cascades on provider delete.

pub struct SyncHistoryRepository<'a> {
    db: &'a Database,
}

impl<'a> SyncHistoryRepository<'a> {
    pub fn new(db: &'a Database) -> Self {
        Self { db }
    }

    /// Record the start of a sync attempt. Returns the new row id.
    pub fn record_start(
        &self,
        provider_id: &str,
        direction: &str,
    ) -> Result<String, AppError> {
        let id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();
        let conn = self.db.connection();
        conn.execute(
            "INSERT INTO sync_history (id, provider_id, direction, status, started_at)
             VALUES (?1, ?2, ?3, 'in_progress', ?4)",
            params![id, provider_id, direction, now],
        )
        .map_err(|e| AppError::Config(format!("Failed to record sync start: {}", e)))?;
        Ok(id)
    }

    /// Mark an in-progress history row finished. status ∈ success/error/cancelled.
    pub fn finish(
        &self,
        history_id: &str,
        status: &str,
        bytes_transferred: Option<i64>,
        skills_count: Option<i64>,
        error_message: Option<&str>,
    ) -> Result<(), AppError> {
        let now = chrono::Utc::now().to_rfc3339();
        let conn = self.db.connection();
        conn.execute(
            "UPDATE sync_history SET status = ?1, finished_at = ?2, bytes_transferred = ?3,
                                     skills_count = ?4, error_message = ?5
             WHERE id = ?6",
            params![status, now, bytes_transferred, skills_count, error_message, history_id],
        )
        .map_err(|e| AppError::Config(format!("Failed to finish sync history: {}", e)))?;
        Ok(())
    }

    /// List history rows for a provider, most recent first. Bounded by `limit`.
    pub fn list_for_provider(
        &self,
        provider_id: &str,
        limit: usize,
    ) -> Result<Vec<SyncHistory>, AppError> {
        let conn = self.db.connection();
        let mut stmt = conn
            .prepare(
                "SELECT id, provider_id, direction, status, started_at, finished_at,
                        bytes_transferred, skills_count, error_message
                 FROM sync_history WHERE provider_id = ?1
                 ORDER BY started_at DESC LIMIT ?2",
            )
            .map_err(|e| AppError::Config(format!("Failed to prepare history list: {}", e)))?;
        let rows = stmt
            .query_map(params![provider_id, limit as i64], |row| {
                Ok(SyncHistory {
                    id: row.get(0)?,
                    provider_id: row.get(1)?,
                    direction: row.get(2)?,
                    status: row.get(3)?,
                    started_at: row.get(4)?,
                    finished_at: row.get(5)?,
                    bytes_transferred: row.get(6)?,
                    skills_count: row.get(7)?,
                    error_message: row.get(8)?,
                })
            })
            .map_err(|e| AppError::Config(format!("Failed to query history: {}", e)))?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(|e| AppError::Config(format!("Failed to read history: {}", e)))
    }

    /// List the most recent history rows across all providers, bounded by `limit`.
    pub fn list_recent(&self, limit: usize) -> Result<Vec<SyncHistory>, AppError> {
        let conn = self.db.connection();
        let mut stmt = conn
            .prepare(
                "SELECT id, provider_id, direction, status, started_at, finished_at,
                        bytes_transferred, skills_count, error_message
                 FROM sync_history ORDER BY started_at DESC LIMIT ?1",
            )
            .map_err(|e| AppError::Config(format!("Failed to prepare recent history: {}", e)))?;
        let rows = stmt
            .query_map(params![limit as i64], |row| {
                Ok(SyncHistory {
                    id: row.get(0)?,
                    provider_id: row.get(1)?,
                    direction: row.get(2)?,
                    status: row.get(3)?,
                    started_at: row.get(4)?,
                    finished_at: row.get(5)?,
                    bytes_transferred: row.get(6)?,
                    skills_count: row.get(7)?,
                    error_message: row.get(8)?,
                })
            })
            .map_err(|e| AppError::Config(format!("Failed to query recent: {}", e)))?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(|e| AppError::Config(format!("Failed to read recent: {}", e)))
    }

    /// Wipe every history row for a provider. Returns the count
    /// removed. Drives the "Clear history" button in Settings.
    pub fn clear_for_provider(&self, provider_id: &str) -> Result<usize, AppError> {
        let conn = self.db.connection();
        let affected = conn
            .execute(
                "DELETE FROM sync_history WHERE provider_id = ?1",
                params![provider_id],
            )
            .map_err(|e| AppError::Config(format!("Failed to clear history: {}", e)))?;
        Ok(affected)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    fn create_test_database() -> Database {
        let temp = NamedTempFile::new().unwrap();
        Database::new(&temp.path().to_path_buf()).unwrap()
    }

    #[test]
    fn test_database_initialization() {
        let db = create_test_database();
        let conn = db.connection();

        let count: i64 = conn
            .query_row(
                "SELECT count(*) FROM sqlite_master WHERE type='table' AND name='skills'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn test_schema_version_set() {
        let db = create_test_database();
        let conn = db.connection();
        let version: u32 = conn
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .unwrap();
        assert_eq!(version, LATEST_VERSION);
    }

    #[test]
    fn test_skills_repository_upsert() {
        let db = create_test_database();
        let repo = SkillsRepository::new(&db);

        let skill = Skill {
            id: "test-skill-1".to_string(),
            name: "test-skill".to_string(),
            path_hash: "hash1234".to_string(),
            library_path: "/test/skill".to_string(),
            original_source_path: Some("/source/skill".to_string()),
            original_git_url: None,
            original_git_subpath: None,
            group: "default".to_string(),
            description: "Test skill".to_string(),
            frontmatter: HashMap::new(),
            created_at: "2024-01-01T00:00:00Z".to_string(),
            mtime_ms: 12345678,
            source_type: SkillSourceType::LocalFolder,
            is_deleted: false,
            content_hash: None,
            source_revision: None,
            source_remote_revision: None,
            source_update_status: Default::default(),
        };

        repo.upsert(&skill).unwrap();
        let installed = repo.get_installed().unwrap();
        assert_eq!(installed.len(), 1);
        assert_eq!(installed[0].name, "test-skill");

        repo.mark_installed("test-skill-1").unwrap();
        let installed = repo.get_installed().unwrap();
        assert_eq!(installed.len(), 1);
        assert_eq!(installed[0].name, "test-skill");
    }

    #[test]
    fn test_config_repository() {
        let db = create_test_database();
        let repo = ConfigRepository::new(&db);

        repo.set("test.key", "test.value").unwrap();
        let value = repo.get("test.key").unwrap();
        assert_eq!(value, Some("test.value".to_string()));

        let all = repo.get_all().unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all.get("test.key"), Some(&"test.value".to_string()));
    }

    #[test]
    fn test_audit_repository() {
        let db = create_test_database();
        let repo = AuditRepository::new(&db);

        repo.log("install", "skill-1", None, true, None).unwrap();
        repo.log(
            "delete",
            "skill-2",
            None,
            false,
            Some("Not found".to_string()),
        )
        .unwrap();

        let logs = repo.get_logs(10).unwrap();
        assert_eq!(logs.len(), 2);
        assert_eq!(logs[0].action, "delete");
        assert_eq!(logs[1].action, "install");
    }

    #[test]
    fn test_update_content_hash() {
        let db = create_test_database();
        let repo = SkillsRepository::new(&db);

        let skill = Skill {
            id: "hash-test".to_string(),
            name: "hash-test".to_string(),
            path_hash: "abc".to_string(),
            library_path: "/test".to_string(),
            original_source_path: None,
            original_git_url: None,
            original_git_subpath: None,
            group: "default".to_string(),
            description: "".to_string(),
            frontmatter: HashMap::new(),
            created_at: "2024-01-01T00:00:00Z".to_string(),
            mtime_ms: 0,
            source_type: SkillSourceType::LocalFolder,
            is_deleted: false,
            content_hash: None,
            source_revision: None,
            source_remote_revision: None,
            source_update_status: Default::default(),
        };

        repo.upsert(&skill).unwrap();
        repo.update_content_hash("hash-test", "sha256abc123")
            .unwrap();

        let conn = db.connection();
        let hash: Option<String> = conn
            .query_row(
                "SELECT content_hash FROM skills WHERE id = ?1",
                params!["hash-test"],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(hash, Some("sha256abc123".to_string()));
    }

    #[test]
    fn test_get_all_active_includes_content_hash() {
        // 根因 5 回归测试：get_all_active 必须读出 content_hash 列。
        // 当前实现 SELECT 列表漏 content_hash + row_to_skill 写死 None → 必红。
        let db = create_test_database();
        let repo = SkillsRepository::new(&db);

        let skill = Skill {
            id: "hash-active".to_string(),
            name: "hash-active-skill".to_string(),
            path_hash: "h".to_string(),
            library_path: "/test".to_string(),
            original_source_path: None,
            original_git_url: None,
            original_git_subpath: None,
            group: "default".to_string(),
            description: "".to_string(),
            frontmatter: HashMap::new(),
            created_at: "2024-01-01T00:00:00Z".to_string(),
            mtime_ms: 0,
            source_type: SkillSourceType::LocalFolder,
            is_deleted: false,
            content_hash: Some("sha256:h_abc".to_string()),
            source_revision: None,
            source_remote_revision: None,
            source_update_status: Default::default(),
        };

        repo.upsert(&skill).unwrap();

        let active = repo.get_all_active().unwrap();
        assert_eq!(active.len(), 1);
        assert_eq!(
            active[0].content_hash.as_deref(),
            Some("sha256:h_abc"),
            "get_all_active 必须读出 content_hash 列（修复 SELECT 列表漏列 + row_to_skill 写死 None）"
        );
    }

    #[test]
    fn test_scan_upsert_and_diff() {
        let db = create_test_database();
        let repo = SkillsRepository::new(&db);

        let skill = Skill {
            id: "scan-1".to_string(),
            name: "scanned-skill".to_string(),
            path_hash: "hash1".to_string(),
            library_path: "/test".to_string(),
            original_source_path: Some("/source".to_string()),
            original_git_url: None,
            original_git_subpath: None,
            group: "default".to_string(),
            description: "Desc".to_string(),
            frontmatter: HashMap::new(),
            created_at: "2024-01-01T00:00:00Z".to_string(),
            mtime_ms: 0,
            source_type: SkillSourceType::LocalFolder,
            is_deleted: false,
            content_hash: None,
            source_revision: None,
            source_remote_revision: None,
            source_update_status: Default::default(),
        };

        let ts1 = "2024-01-01T00:00:00Z";
        let ts2 = "2024-01-02T00:00:00Z";

        // First scan: skill is new
        repo.upsert_with_scan(&skill, ts1).unwrap();
        let new_skills = repo.get_new_skills(ts1).unwrap();
        assert_eq!(new_skills.len(), 1);

        // Second scan: skill is updated (not new)
        repo.upsert_with_scan(&skill, ts2).unwrap();
        let updated = repo.get_updated_skills(ts2).unwrap();
        assert_eq!(updated.len(), 1);

        // Mark missing as deleted
        let deleted = repo.mark_missing_as_deleted(ts2).unwrap();
        assert!(deleted.is_empty()); // scan-1 was seen at ts2, so not deleted
    }

    #[test]
    fn test_mark_missing_as_deleted_is_soft_delete() {
        let db = create_test_database();
        let repo = SkillsRepository::new(&db);

        let skill = Skill {
            id: "soft-delete-test".to_string(),
            name: "soft-delete-test".to_string(),
            path_hash: "hash_sd".to_string(),
            library_path: "/test".to_string(),
            original_source_path: Some("/source".to_string()),
            original_git_url: None,
            original_git_subpath: None,
            group: "default".to_string(),
            description: "Soft delete test".to_string(),
            frontmatter: HashMap::new(),
            created_at: "2024-01-01T00:00:00Z".to_string(),
            mtime_ms: 0,
            source_type: SkillSourceType::LocalFolder,
            is_deleted: false,
            content_hash: None,
            source_revision: None,
            source_remote_revision: None,
            source_update_status: Default::default(),
        };

        // Use upsert_with_scan so we control last_seen_at
        let ts_insert = "2024-01-01T00:00:00Z";
        repo.upsert_with_scan(&skill, ts_insert).unwrap();
        repo.mark_installed("soft-delete-test").unwrap();

        // Simulate a later scan where this skill is NOT seen
        let ts_later = "2024-06-01T00:00:00Z";
        let deleted_ids = repo.mark_missing_as_deleted(ts_later).unwrap();
        assert_eq!(deleted_ids.len(), 1, "Skill should be soft-deleted");
        assert_eq!(deleted_ids[0], "soft-delete-test");

        // Verify: row still exists but is_deleted = 1
        let conn = db.connection();
        let is_deleted: i32 = conn
            .query_row(
                "SELECT is_deleted FROM skills WHERE id = ?1",
                params!["soft-delete-test"],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(
            is_deleted, 1,
            "Skill should be soft-deleted (is_deleted=1), not hard-deleted"
        );

        // Verify: get_installed() should NOT return it
        let installed = repo.get_installed().unwrap();
        assert!(
            installed.is_empty(),
            "Soft-deleted skill should not appear as installed"
        );
    }

    #[test]
    fn test_delete_by_name_hard_delete_cascades_links() {
        let db = create_test_database();
        let skills_repo = SkillsRepository::new(&db);
        let tools_repo = ToolsRepository::new(&db);
        let links_repo = LinksRepository::new(&db);

        let skill = Skill {
            id: "hard-del-1".to_string(),
            name: "hard-del".to_string(),
            path_hash: "h1".to_string(),
            library_path: "/test".to_string(),
            original_source_path: None,
            original_git_url: None,
            original_git_subpath: None,
            group: "default".to_string(),
            description: "".to_string(),
            frontmatter: HashMap::new(),
            created_at: "2024-01-01T00:00:00Z".to_string(),
            mtime_ms: 0,
            source_type: SkillSourceType::LocalFolder,
            is_deleted: false,
            content_hash: None,
            source_revision: None,
            source_remote_revision: None,
            source_update_status: Default::default(),
        };
        skills_repo.upsert(&skill).unwrap();

        let tool = Tool {
            id: "tool-a".to_string(),
            name: "Tool A".to_string(),
            path: "/tools/a".to_string(),
            enabled: true,
            is_custom: false,
        };
        tools_repo.upsert(&tool).unwrap();
        links_repo.link("tool-a", "hard-del-1").unwrap();

        skills_repo.delete_by_name("hard-del").unwrap();

        let conn = db.connection();
        let skill_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM skills WHERE id = ?1",
                params!["hard-del-1"],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(skill_count, 0, "Skill row should be physically removed");

        let link_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM tool_skill_links WHERE skill_id = ?1",
                params!["hard-del-1"],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(link_count, 0, "tool_skill_links should be cascade-deleted");
    }

    #[test]
    fn test_links_repository_is_active() {
        let db = create_test_database();
        let tools_repo = ToolsRepository::new(&db);
        let skills_repo = SkillsRepository::new(&db);
        let links_repo = LinksRepository::new(&db);

        let tool = Tool {
            id: "tool-x".to_string(),
            name: "Tool X".to_string(),
            path: "/x".to_string(),
            enabled: true,
            is_custom: false,
        };
        tools_repo.upsert(&tool).unwrap();

        let skill = Skill {
            id: "skill-y".to_string(),
            name: "skill-y".to_string(),
            path_hash: "h".to_string(),
            library_path: "/y".to_string(),
            original_source_path: None,
            original_git_url: None,
            original_git_subpath: None,
            group: "default".to_string(),
            description: "".to_string(),
            frontmatter: HashMap::new(),
            created_at: "2024-01-01T00:00:00Z".to_string(),
            mtime_ms: 0,
            source_type: SkillSourceType::LocalFolder,
            is_deleted: false,
            content_hash: None,
            source_revision: None,
            source_remote_revision: None,
            source_update_status: Default::default(),
        };
        skills_repo.upsert(&skill).unwrap();

        // No link yet — should report inactive.
        assert!(!links_repo.is_active("tool-x", "skill-y").unwrap());

        // After link(): active.
        links_repo.link("tool-x", "skill-y").unwrap();
        assert!(links_repo.is_active("tool-x", "skill-y").unwrap());

        // After unlink(): inactive again.
        links_repo.unlink("tool-x", "skill-y").unwrap();
        assert!(!links_repo.is_active("tool-x", "skill-y").unwrap());

        // A different tool_id should not see the link as active.
        links_repo.link("tool-x", "skill-y").unwrap();
        assert!(!links_repo.is_active("tool-other", "skill-y").unwrap());
    }

    // ── Tags Repository ──────────────────────────────────────────────

    /// Insert a real skill row so FK on skill_tags(skill_id) is satisfied.
    fn seed_skill(db: &Database, id: &str) {
        let skill = Skill {
            id: id.to_string(),
            name: id.to_string(),
            path_hash: format!("hash-{id}"),
            library_path: format!("/test/{id}"),
            original_source_path: None,
            original_git_url: None,
            original_git_subpath: None,
            group: "default".to_string(),
            description: "".to_string(),
            frontmatter: HashMap::new(),
            created_at: "2024-01-01T00:00:00Z".to_string(),
            mtime_ms: 0,
            source_type: SkillSourceType::LocalFolder,
            is_deleted: false,
            content_hash: None,
            source_revision: None,
            source_remote_revision: None,
            source_update_status: Default::default(),
        };
        SkillsRepository::new(db).upsert(&skill).unwrap();
    }

    #[test]
    fn test_tags_migration_creates_tables() {
        let db = create_test_database();
        let conn = db.connection();
        for table in ["tags", "skill_tags"] {
            let count: i64 = conn
                .query_row(
                    "SELECT count(*) FROM sqlite_master WHERE type='table' AND name=?1",
                    params![table],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(count, 1, "expected table {table} to exist after migration");
        }
    }

    #[test]
    fn test_tags_repository_create_and_get() {
        // Arrange
        let db = create_test_database();
        let repo = TagsRepository::new(&db);

        // Act
        let tag = repo
            .create("rust", Some("#dea584"), Some("Rust lang skills"))
            .unwrap();

        // Assert: returned value is well-formed
        assert!(!tag.id.is_empty(), "id should be auto-generated");
        assert_eq!(tag.name, "rust");
        assert_eq!(tag.color.as_deref(), Some("#dea584"));
        assert_eq!(tag.description.as_deref(), Some("Rust lang skills"));
        assert!(!tag.created_at.is_empty());

        // Assert: get_by_id round-trips
        let fetched = repo.get_by_id(&tag.id).unwrap().expect("tag should exist");
        assert_eq!(fetched.name, "rust");
    }

    #[test]
    fn test_tags_repository_create_duplicate_name_fails() {
        let db = create_test_database();
        let repo = TagsRepository::new(&db);

        repo.create("dup", None, None).unwrap();
        let err = repo.create("dup", None, None).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("UNIQUE") || msg.contains("unique") || msg.contains("dup"),
            "expected unique-constraint error, got: {msg}"
        );
    }

    #[test]
    fn test_tags_repository_list_returns_all_sorted_by_name() {
        let db = create_test_database();
        let repo = TagsRepository::new(&db);
        repo.create("zeta", None, None).unwrap();
        repo.create("alpha", None, None).unwrap();
        repo.create("mike", None, None).unwrap();

        let tags = repo.list().unwrap();
        let names: Vec<&str> = tags.iter().map(|t| t.name.as_str()).collect();
        assert_eq!(names, vec!["alpha", "mike", "zeta"]);
    }

    #[test]
    fn test_tags_repository_update() {
        let db = create_test_database();
        let repo = TagsRepository::new(&db);
        let tag = repo
            .create("old-name", Some("#000000"), Some("old"))
            .unwrap();

        // Convention: None = leave unchanged, Some("") = clear the field.
        // Update name + color, then explicitly clear description.
        repo.update(&tag.id, Some("new-name"), Some("#ffffff"), Some(""))
            .unwrap();

        let fetched = repo.get_by_id(&tag.id).unwrap().unwrap();
        assert_eq!(fetched.name, "new-name");
        assert_eq!(fetched.color.as_deref(), Some("#ffffff"));
        assert_eq!(
            fetched.description, None,
            "passing Some(\"\") clears the field"
        );
    }

    #[test]
    fn test_tags_repository_delete_cascades_skill_tags() {
        let db = create_test_database();
        let tags_repo = TagsRepository::new(&db);
        seed_skill(&db, "skill-a");
        seed_skill(&db, "skill-b");

        let tag = tags_repo.create("ephemeral", None, None).unwrap();
        tags_repo.attach("skill-a", &tag.id).unwrap();
        tags_repo.attach("skill-b", &tag.id).unwrap();

        // Sanity: 2 links exist (scoped so the MutexGuard drops before delete)
        {
            let conn = db.connection();
            let count: i64 = conn
                .query_row(
                    "SELECT count(*) FROM skill_tags WHERE tag_id = ?1",
                    params![&tag.id],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(count, 2);
        }

        // Act
        tags_repo.delete(&tag.id).unwrap();

        // Assert: tag gone + links cascade
        assert!(tags_repo.get_by_id(&tag.id).unwrap().is_none());
        let conn = db.connection();
        let count_after: i64 = conn
            .query_row("SELECT count(*) FROM skill_tags", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count_after, 0, "skill_tags should cascade on tag delete");
    }

    #[test]
    fn test_tags_repository_attach_and_detach() {
        let db = create_test_database();
        let tags_repo = TagsRepository::new(&db);
        seed_skill(&db, "skill-1");

        let tag = tags_repo.create("important", None, None).unwrap();
        tags_repo.attach("skill-1", &tag.id).unwrap();

        // After attach: skill sees the tag
        let for_skill = tags_repo.list_tags_for_skill("skill-1").unwrap();
        assert_eq!(for_skill.len(), 1);
        assert_eq!(for_skill[0].name, "important");

        // Detach
        tags_repo.detach("skill-1", &tag.id).unwrap();
        let for_skill_after = tags_repo.list_tags_for_skill("skill-1").unwrap();
        assert!(for_skill_after.is_empty());
    }

    #[test]
    fn test_tags_repository_attach_is_idempotent() {
        // Re-attaching the same (skill, tag) should not error or duplicate
        let db = create_test_database();
        let tags_repo = TagsRepository::new(&db);
        seed_skill(&db, "skill-1");
        let tag = tags_repo.create("once", None, None).unwrap();

        tags_repo.attach("skill-1", &tag.id).unwrap();
        tags_repo.attach("skill-1", &tag.id).unwrap();

        let conn = db.connection();
        let count: i64 = conn
            .query_row(
                "SELECT count(*) FROM skill_tags WHERE skill_id=?1 AND tag_id=?2",
                params!["skill-1", &tag.id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn test_tags_repository_bulk_attach() {
        let db = create_test_database();
        let tags_repo = TagsRepository::new(&db);
        for s in ["s1", "s2", "s3"] {
            seed_skill(&db, s);
        }
        let tag = tags_repo.create("group", None, None).unwrap();

        let result = tags_repo
            .bulk_attach(&["s1", "s2", "s3", "nonexistent"], &tag.id)
            .unwrap();

        // 3 valid skills should be attached, 1 should be skipped
        assert_eq!(result.attached, 3, "only existing skills should be linked");
        assert_eq!(
            result.skipped, 1,
            "nonexistent skill_id must be counted as skipped"
        );

        let skills = tags_repo.list_skills_for_tag(&tag.id).unwrap();
        let ids: Vec<&str> = skills.iter().map(|s| s.as_str()).collect();
        assert!(ids.contains(&"s1") && ids.contains(&"s2") && ids.contains(&"s3"));
    }

    #[test]
    fn test_tags_repository_delete_skill_cascades_skill_tags() {
        // Mirrors test_tags_repository_delete_cascades_skill_tags but from the
        // skill side: removing a skill should drop its tag links.
        let db = create_test_database();
        let tags_repo = TagsRepository::new(&db);
        seed_skill(&db, "doomed");
        let tag = tags_repo.create("t", None, None).unwrap();
        tags_repo.attach("doomed", &tag.id).unwrap();

        // Drop guard before calling repo methods.
        {
            let conn = db.connection();
            conn.execute("DELETE FROM skills WHERE id='doomed'", [])
                .unwrap();
        }

        let for_skill = tags_repo.list_tags_for_skill("doomed").unwrap();
        assert!(
            for_skill.is_empty(),
            "deleting a skill must cascade to skill_tags"
        );
    }

    // ── Coverage gaps flagged by tdd-guide / code-reviewer ──────────

    #[test]
    fn test_tags_repository_update_none_leaves_other_fields_unchanged() {
        // Arrange
        let db = create_test_database();
        let repo = TagsRepository::new(&db);
        let tag = repo
            .create("keep", Some("#111111"), Some("keep me"))
            .unwrap();

        // Act: only update name; pass None for color and description
        repo.update(&tag.id, Some("renamed"), None, None).unwrap();

        // Assert: name changed, color/description preserved
        let fetched = repo.get_by_id(&tag.id).unwrap().unwrap();
        assert_eq!(fetched.name, "renamed");
        assert_eq!(fetched.color.as_deref(), Some("#111111"));
        assert_eq!(fetched.description.as_deref(), Some("keep me"));
    }

    #[test]
    fn test_tags_repository_update_empty_string_clears_nullable_columns() {
        // `tags.name` is NOT NULL — can't be cleared to NULL, the SQL layer
        // rejects the update. `color` and `description` are nullable and
        // follow the "Some('') clears" convention.
        let db = create_test_database();
        let repo = TagsRepository::new(&db);
        let tag = repo.create("n", Some("#000000"), Some("d")).unwrap();

        // name=Some("") errors because of NOT NULL — assert that and move on
        let err = repo.update(&tag.id, Some(""), None, None).unwrap_err();
        assert!(
            err.to_string().contains("NOT NULL"),
            "expected NOT NULL rejection, got: {err}"
        );

        // color and description clear on Some("")
        repo.update(&tag.id, None, Some(""), Some("")).unwrap();
        let fetched = repo.get_by_id(&tag.id).unwrap().unwrap();
        assert_eq!(fetched.name, "n", "name must be unchanged");
        assert_eq!(
            fetched.color, None,
            "empty color string should clear to NULL"
        );
        assert_eq!(fetched.description, None);
    }

    #[test]
    fn test_tags_repository_delete_nonexistent_is_noop() {
        let db = create_test_database();
        let repo = TagsRepository::new(&db);

        // Act + Assert: no error, no data corruption
        repo.delete("ghost-id").unwrap();
        assert!(repo.list().unwrap().is_empty());
    }

    #[test]
    fn test_tags_repository_detach_nonexistent_link_is_noop() {
        let db = create_test_database();
        let repo = TagsRepository::new(&db);
        seed_skill(&db, "s1");
        let tag = repo.create("t", None, None).unwrap();

        // Never attached, no error
        repo.detach("s1", &tag.id).unwrap();
        // skill that doesn't exist
        repo.detach("ghost", &tag.id).unwrap();
        assert!(repo.list_tags_for_skill("s1").unwrap().is_empty());
    }

    #[test]
    fn test_tags_repository_get_by_name_hit_and_miss() {
        let db = create_test_database();
        let repo = TagsRepository::new(&db);
        repo.create("frontend", None, None).unwrap();

        let hit = repo
            .get_by_name("frontend")
            .unwrap()
            .expect("tag should be found");
        assert_eq!(hit.name, "frontend");

        let miss = repo.get_by_name("backend").unwrap();
        assert!(
            miss.is_none(),
            "unknown name must return Ok(None), not error"
        );
    }

    #[test]
    fn test_tags_repository_list_skills_for_tag_empty_and_missing() {
        let db = create_test_database();
        let repo = TagsRepository::new(&db);
        let tag = repo.create("lonely", None, None).unwrap();

        // Existing tag, zero links
        assert!(repo.list_skills_for_tag(&tag.id).unwrap().is_empty());
        // Non-existent tag
        assert!(repo.list_skills_for_tag("ghost-id").unwrap().is_empty());
    }

    #[test]
    fn test_tags_repository_list_all_skill_tags_groups_by_skill() {
        let db = create_test_database();
        let repo = TagsRepository::new(&db);
        for s in ["s1", "s2"] {
            seed_skill(&db, s);
        }
        let t_a = repo.create("a", None, None).unwrap();
        let t_b = repo.create("b", None, None).unwrap();
        repo.attach("s1", &t_a.id).unwrap();
        repo.attach("s1", &t_b.id).unwrap();
        repo.attach("s2", &t_a.id).unwrap();

        // Act
        let rows = repo.list_all_skill_tags().unwrap();

        // Assert: 3 (skill_id, tag) rows
        assert_eq!(rows.len(), 3);
        let s1_tags: Vec<&str> = rows
            .iter()
            .filter(|(sid, _)| sid == "s1")
            .map(|(_, t)| t.name.as_str())
            .collect();
        assert_eq!(s1_tags.len(), 2);
        assert!(s1_tags.contains(&"a") && s1_tags.contains(&"b"));
    }

    #[test]
    fn test_create_rejects_empty_name() {
        let db = create_test_database();
        let repo = TagsRepository::new(&db);
        let err = repo.create("", None, None).unwrap_err();
        assert!(err.to_string().contains("empty"), "got: {err}");
    }

    #[test]
    fn test_create_rejects_overlong_name() {
        let db = create_test_database();
        let repo = TagsRepository::new(&db);
        let big = "x".repeat(MAX_TAG_NAME_LEN + 1);
        let err = repo.create(&big, None, None).unwrap_err();
        assert!(err.to_string().contains("too long"), "got: {err}");
    }

    #[test]
    fn test_create_rejects_control_chars_in_name() {
        let db = create_test_database();
        let repo = TagsRepository::new(&db);
        let bad = "rust\ngood";
        let err = repo.create(bad, None, None).unwrap_err();
        assert!(err.to_string().contains("control"), "got: {err}");
    }

    // ── Sync provider / history tests ──────────────────────────────────

    /// Seed a sync_providers row (mirrors seed_skill for tags tests).
    fn seed_provider(db: &Database, id: &str, name: &str) {
        SyncProvidersRepository::new(db)
            .create(name, "webdav", r#"{"url":"http://example.com"}"#, true)
            .unwrap();
        // Sanity: id is generated; we just need the row to exist.
        let _ = id;
    }

    #[test]
    fn test_sync_migration_creates_tables() {
        let db = create_test_database();
        let conn = db.connection();

        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name IN ('sync_providers','sync_history')",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 2, "both sync tables should exist after migration");

        let idx_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='index' AND name IN
                 ('idx_sync_history_provider','idx_sync_history_started')",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(idx_count, 2, "both sync indexes should exist after migration");
    }

    #[test]
    fn test_sync_providers_repository_crud() {
        let db = create_test_database();
        let repo = SyncProvidersRepository::new(&db);

        // Create
        let p = repo
            .create("personal", "webdav", r#"{"url":"http://x"}"#, true)
            .unwrap();
        assert_eq!(p.name, "personal");
        assert_eq!(p.kind, "webdav");
        assert!(p.enabled);
        assert!(p.last_sync_at.is_none());

        // Get
        let fetched = repo.get(&p.id).unwrap().unwrap();
        assert_eq!(fetched.name, "personal");
        assert_eq!(fetched.config_json, r#"{"url":"http://x"}"#);

        // List sorted by name
        let _ = repo
            .create("work", "github_zip", r#"{"repo":"a/b"}"#, true)
            .unwrap();
        let list = repo.list(false).unwrap();
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].name, "personal"); // sorted
        assert_eq!(list[1].name, "work");

        // enabled_only filter
        repo.update(&p.id, None, None, Some(false)).unwrap();
        let enabled = repo.list(true).unwrap();
        assert_eq!(enabled.len(), 1);
        assert_eq!(enabled[0].name, "work");

        // Delete
        repo.delete(&p.id).unwrap();
        assert!(repo.get(&p.id).unwrap().is_none());
    }

    #[test]
    fn test_sync_providers_rejects_invalid_name() {
        let db = create_test_database();
        let repo = SyncProvidersRepository::new(&db);

        // Empty
        let err = repo.create("", "webdav", "{}", true).unwrap_err();
        assert!(err.to_string().contains("empty"), "got: {err}");

        // Whitespace-only
        let err = repo.create("   ", "webdav", "{}", true).unwrap_err();
        assert!(err.to_string().contains("empty"), "got: {err}");

        // Control char
        let err = repo.create("ok\nbad", "webdav", "{}", true).unwrap_err();
        assert!(err.to_string().contains("control"), "got: {err}");

        // Overlong
        let long = "a".repeat(MAX_PROVIDER_NAME_LEN + 1);
        let err = repo.create(&long, "webdav", "{}", true).unwrap_err();
        assert!(err.to_string().contains("too long"), "got: {err}");

        // Empty config
        let err = repo.create("ok", "webdav", "", true).unwrap_err();
        assert!(err.to_string().contains("config_json"), "got: {err}");
    }

    #[test]
    fn test_sync_providers_unique_name_constraint() {
        let db = create_test_database();
        let repo = SyncProvidersRepository::new(&db);

        repo.create("dup", "webdav", "{}", true).unwrap();
        let err = repo.create("dup", "webdav", "{}", true).unwrap_err();
        assert!(
            err.to_string().contains("already exists"),
            "got: {err}"
        );

        // Trim before uniqueness check
        let err = repo.create("  dup  ", "webdav", "{}", true).unwrap_err();
        assert!(err.to_string().contains("already exists"), "got: {err}");
    }

    #[test]
    fn test_sync_providers_update_field_conventions() {
        let db = create_test_database();
        let repo = SyncProvidersRepository::new(&db);
        let p = repo
            .create("orig", "webdav", r#"{"url":"http://a"}"#, true)
            .unwrap();

        // None leaves fields alone
        repo.update(&p.id, None, None, None).unwrap();
        let after = repo.get(&p.id).unwrap().unwrap();
        assert_eq!(after.name, "orig");
        assert_eq!(after.config_json, r#"{"url":"http://a"}"#);
        assert!(after.enabled);

        // Some("...") updates
        repo.update(&p.id, Some("renamed"), Some(r#"{"url":"http://b"}"#), Some(false))
            .unwrap();
        let after = repo.get(&p.id).unwrap().unwrap();
        assert_eq!(after.name, "renamed");
        assert_eq!(after.config_json, r#"{"url":"http://b"}"#);
        assert!(!after.enabled);
    }

    #[test]
    fn test_sync_providers_record_last_sync() {
        let db = create_test_database();
        let repo = SyncProvidersRepository::new(&db);
        let p = repo.create("p", "webdav", "{}", true).unwrap();

        repo.record_last_sync(&p.id, "success", None).unwrap();
        let after = repo.get(&p.id).unwrap().unwrap();
        assert_eq!(after.last_sync_status.as_deref(), Some("success"));
        assert!(after.last_sync_at.is_some());
        assert!(after.last_sync_error.is_none());

        repo.record_last_sync(&p.id, "error", Some("network down"))
            .unwrap();
        let after = repo.get(&p.id).unwrap().unwrap();
        assert_eq!(after.last_sync_status.as_deref(), Some("error"));
        assert_eq!(after.last_sync_error.as_deref(), Some("network down"));
    }

    #[test]
    fn test_sync_providers_delete_cascades_history() {
        let db = create_test_database();
        let prov_repo = SyncProvidersRepository::new(&db);
        let hist_repo = SyncHistoryRepository::new(&db);

        let p = prov_repo.create("p", "webdav", "{}", true).unwrap();
        let h1 = hist_repo.record_start(&p.id, "upload").unwrap();
        hist_repo
            .finish(&h1, "success", Some(1024), Some(3), None)
            .unwrap();
        let h2 = hist_repo.record_start(&p.id, "download").unwrap();
        hist_repo
            .finish(&h2, "error", None, None, Some("boom"))
            .unwrap();

        // Sanity: 2 history rows attached.
        assert_eq!(hist_repo.list_for_provider(&p.id, 10).unwrap().len(), 2);

        prov_repo.delete(&p.id).unwrap();

        // CASCADE: rows must be gone.
        let remaining = hist_repo.list_for_provider(&p.id, 10).unwrap();
        assert!(remaining.is_empty(), "history must cascade-delete");
    }

    #[test]
    fn test_sync_history_in_progress_then_finish() {
        let db = create_test_database();
        let prov_repo = SyncProvidersRepository::new(&db);
        let hist_repo = SyncHistoryRepository::new(&db);
        let p = prov_repo.create("p", "webdav", "{}", true).unwrap();

        let h = hist_repo.record_start(&p.id, "upload").unwrap();
        let row = hist_repo
            .list_for_provider(&p.id, 10)
            .unwrap()
            .into_iter()
            .find(|r| r.id == h)
            .expect("row should exist");
        assert_eq!(row.status, "in_progress");
        assert!(row.finished_at.is_none());

        hist_repo
            .finish(&h, "success", Some(2048), Some(5), None)
            .unwrap();
        let row = hist_repo
            .list_for_provider(&p.id, 10)
            .unwrap()
            .into_iter()
            .find(|r| r.id == h)
            .unwrap();
        assert_eq!(row.status, "success");
        assert!(row.finished_at.is_some());
        assert_eq!(row.bytes_transferred, Some(2048));
        assert_eq!(row.skills_count, Some(5));
    }

    #[test]
    fn test_sync_history_recent_and_limit() {
        let db = create_test_database();
        let prov_repo = SyncProvidersRepository::new(&db);
        let hist_repo = SyncHistoryRepository::new(&db);
        let p = prov_repo.create("p", "webdav", "{}", true).unwrap();

        // Record 5 rows
        for _ in 0..5 {
            let h = hist_repo.record_start(&p.id, "upload").unwrap();
            hist_repo.finish(&h, "success", None, None, None).unwrap();
        }

        // list_for_provider with limit=3
        let rows = hist_repo.list_for_provider(&p.id, 3).unwrap();
        assert_eq!(rows.len(), 3);

        // list_recent limit=2
        let recent = hist_repo.list_recent(2).unwrap();
        assert_eq!(recent.len(), 2);
    }

    #[test]
    fn test_sync_history_clear_for_provider() {
        let db = create_test_database();
        let prov_repo = SyncProvidersRepository::new(&db);
        let hist_repo = SyncHistoryRepository::new(&db);
        let p1 = prov_repo.create("p1", "webdav", "{}", true).unwrap();
        let p2 = prov_repo.create("p2", "webdav", "{}", true).unwrap();

        // 3 rows for p1, 2 for p2
        for _ in 0..3 {
            let h = hist_repo.record_start(&p1.id, "upload").unwrap();
            hist_repo.finish(&h, "success", None, None, None).unwrap();
        }
        for _ in 0..2 {
            let h = hist_repo.record_start(&p2.id, "upload").unwrap();
            hist_repo.finish(&h, "success", None, None, None).unwrap();
        }
        assert_eq!(hist_repo.list_for_provider(&p1.id, 100).unwrap().len(), 3);
        assert_eq!(hist_repo.list_for_provider(&p2.id, 100).unwrap().len(), 2);

        // Clear p1 only — p2 should be untouched.
        let removed = hist_repo.clear_for_provider(&p1.id).unwrap();
        assert_eq!(removed, 3);
        assert!(hist_repo.list_for_provider(&p1.id, 100).unwrap().is_empty());
        assert_eq!(hist_repo.list_for_provider(&p2.id, 100).unwrap().len(), 2);

        // Calling clear again on an empty provider returns 0.
        assert_eq!(hist_repo.clear_for_provider(&p1.id).unwrap(), 0);
    }

    #[test]
    fn test_sync_providers_get_by_name_and_missing() {
        let db = create_test_database();
        let repo = SyncProvidersRepository::new(&db);

        // Miss
        assert!(repo.get_by_name("nope").unwrap().is_none());
        assert!(repo.get("nonexistent-id").unwrap().is_none());

        // Hit
        let p = repo.create("myprov", "webdav", "{}", true).unwrap();
        let by_name = repo.get_by_name("myprov").unwrap().unwrap();
        assert_eq!(by_name.id, p.id);

        // delete with unknown id surfaces error
        let err = repo.delete("does-not-exist").unwrap_err();
        assert!(err.to_string().contains("not found"), "got: {err}");
    }

    #[test]
    fn test_sync_providers_update_missing_returns_error() {
        let db = create_test_database();
        let repo = SyncProvidersRepository::new(&db);
        let err = repo.update("nope", Some("x"), None, None).unwrap_err();
        assert!(err.to_string().contains("not found"), "got: {err}");
    }

    // Touch the helper so it's not flagged dead.
    #[test]
    fn test_seed_provider_helper_inserts_row() {
        let db = create_test_database();
        seed_provider(&db, "ignored", "seeded");
        let p = SyncProvidersRepository::new(&db)
            .get_by_name("seeded")
            .unwrap()
            .expect("seeded row should exist");
        assert_eq!(p.kind, "webdav");
    }
}
