use crate::core::crypto::Crypto;
use crate::core::error::AppError;
use crate::core::models::{AuditEntry, Skill, SkillSourceType, Tool};
use rusqlite::{params, Connection, OptionalExtension};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;

/// Current schema version. Bump this when adding new migration steps.
const LATEST_VERSION: u32 = 4;

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

        conn.execute_batch(
            "PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;",
        )
        .map_err(|e| AppError::Config(format!("Failed to set pragmas: {}", e)))?;

        Self::run_migrations(&conn)?;

        let key_dir = db_path
            .parent()
            .unwrap_or(db_path);
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
            );"
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
            );"
        )
        .map_err(|e| AppError::Config(format!("Failed to create marketplace_cache table: {}", e)))?;
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
            );"
        )
        .map_err(|e| AppError::Config(format!("Failed to create projects table: {}", e)))?;
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

        let now = chrono::Utc::now().to_rfc3339();

        conn.execute(
            "INSERT INTO skills (
                id, name, path_hash, library_path, original_source_path,
                original_git_url, original_git_subpath, group_name, description,
                frontmatter, created_at, mtime_ms, source_type, is_deleted,
                last_seen_at, first_seen_at, is_installed, installed_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18)
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
                installed_at = ?18",
            params![
                skill.id, skill.name, skill.path_hash, skill.library_path,
                skill.original_source_path, skill.original_git_url, skill.original_git_subpath,
                skill.group, skill.description, frontmatter_json, skill.created_at,
                skill.mtime_ms, source_type_str, skill.is_deleted as i32,
                now, skill.created_at,
                1 as i32, now,
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
                        frontmatter, created_at, mtime_ms, source_type, is_deleted
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
                        frontmatter, created_at, mtime_ms, source_type, is_deleted
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
                        frontmatter, created_at, mtime_ms, source_type, is_deleted
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
                skill.id, skill.name, skill.path_hash, skill.library_path,
                skill.original_source_path, skill.original_git_url, skill.original_git_subpath,
                skill.group, skill.description, frontmatter_json, skill.created_at,
                skill.mtime_ms, source_type_str,
                skill.is_deleted as i32, scan_timestamp,
            ],
        )
        .map_err(|e| AppError::Config(format!("Failed to upsert skill: {}", e)))?;

        Ok(())
    }

    pub fn mark_missing_as_deleted(&self, scan_timestamp: &str) -> Result<Vec<String>, AppError> {
        let conn = self.db.connection();
        let mut stmt = conn
            .prepare(
                "DELETE FROM skills WHERE last_seen_at < ?1
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
                        frontmatter, created_at, mtime_ms, source_type, is_deleted
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
                        frontmatter, created_at, mtime_ms, source_type, is_deleted
                 FROM skills
                 WHERE last_seen_at = ?1
                 AND first_seen_at != last_seen_at
                 AND is_deleted = 0
                 ORDER BY name",
            )
            .map_err(|e| AppError::Config(format!("Failed to prepare updated skills query: {}", e)))?;

        Self::query_skills_from_stmt(&mut stmt, [scan_timestamp])
    }

    pub fn get_deleted_skills(&self) -> Result<Vec<Skill>, AppError> {
        let conn = self.db.connection();
        let mut stmt = conn
            .prepare(
                "SELECT id, name, path_hash, library_path, original_source_path,
                        original_git_url, original_git_subpath, group_name, description,
                        frontmatter, created_at, mtime_ms, source_type, is_deleted
                 FROM skills WHERE is_deleted = 1 ORDER BY last_seen_at DESC",
            )
            .map_err(|e| AppError::Config(format!("Failed to prepare deleted skills query: {}", e)))?;

        Self::query_skills_from_stmt(&mut stmt, [])
    }

    pub fn get_by_id(&self, id: &str) -> Result<Option<Skill>, AppError> {
        let conn = self.db.connection();
        let mut stmt = conn
            .prepare(
                "SELECT id, name, path_hash, library_path, original_source_path,
                        original_git_url, original_git_subpath, group_name, description,
                        frontmatter, created_at, mtime_ms, source_type, is_deleted
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
                content_hash: None,
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
                tool.id, tool.name, tool.path,
                tool.enabled as i32, tool.is_custom as i32,
                now, now,
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
            .query_row("SELECT COUNT(*) FROM marketplace_cache", [], |row| row.get(0))
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
            .prepare("SELECT id, name, root_path, created_at, updated_at FROM projects ORDER BY name")
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

        repo.log("install", "skill-1", None, true, None)
            .unwrap();
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
        };

        repo.upsert(&skill).unwrap();
        repo.update_content_hash("hash-test", "sha256abc123").unwrap();

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
}
