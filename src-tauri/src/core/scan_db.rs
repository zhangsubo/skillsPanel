use crate::core::error::AppError;
use crate::core::models::{Skill, SkillSourceType, SkillWithStatus};
use rusqlite::{params, Connection};
use std::collections::HashMap;
use std::path::PathBuf;

pub struct ScanDatabase {
    conn: Connection,
}

impl ScanDatabase {
    pub fn new(db_path: &PathBuf) -> Result<Self, AppError> {
        let conn = Connection::open(db_path)
            .map_err(|e| AppError::Config(format!("Failed to open DB: {}", e)))?;
        Self::init_schema(&conn)?;
        Ok(Self { conn })
    }

    fn init_schema(conn: &Connection) -> Result<(), AppError> {
        conn.execute(
            "CREATE TABLE IF NOT EXISTS skills (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                path_hash TEXT NOT NULL,
                library_path TEXT NOT NULL,
                original_source_path TEXT,
                original_git_url TEXT,
                original_git_subpath TEXT,
                group_name TEXT NOT NULL,
                description TEXT NOT NULL,
                frontmatter TEXT NOT NULL,
                created_at TEXT NOT NULL,
                mtime_ms INTEGER NOT NULL,
                source_type TEXT NOT NULL,
                is_deleted INTEGER NOT NULL DEFAULT 0,
                last_seen_at TEXT NOT NULL,
                first_seen_at TEXT NOT NULL
            )",
            [],
        )
        .map_err(|e| AppError::Config(format!("Failed to create skills table: {}", e)))?;

        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_skills_name ON skills(name)",
            [],
        )
        .map_err(|e| AppError::Config(format!("Failed to create index: {}", e)))?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_skills_source ON skills(original_source_path)",
            [],
        )
        .map_err(|e| AppError::Config(format!("Failed to create index: {}", e)))?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_skills_last_seen ON skills(last_seen_at)",
            [],
        )
        .map_err(|e| AppError::Config(format!("Failed to create index: {}", e)))?;

        Ok(())
    }

    pub fn upsert_skill(&self, skill: &Skill, scan_timestamp: &str) -> Result<(), AppError> {
        let frontmatter_json = serde_json::to_string(&skill.frontmatter)
            .map_err(|e| AppError::Config(format!("Failed to serialize frontmatter: {}", e)))?;

        let source_type_str = match skill.source_type {
            SkillSourceType::Git => "git",
            SkillSourceType::LocalZip => "local-zip",
            SkillSourceType::LocalFolder => "local-folder",
        };

        self.conn
            .execute(
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
        // Hard delete: physically remove records that weren't seen in the latest scan
        let mut stmt = self
            .conn
            .prepare(
                "DELETE FROM skills WHERE last_seen_at < ?1
             RETURNING id",
            )
            .map_err(|e| AppError::Config(format!("Failed to prepare delete query: {}", e)))?;

        let ids: Result<Vec<String>, _> = stmt
            .query_map([scan_timestamp], |row| row.get(0))
            .map_err(|e| AppError::Config(format!("Failed to query deleted skills: {}", e)))?
            .collect();

        Ok(ids.map_err(|e| AppError::Config(format!("Failed to collect deleted skills: {}", e)))?)
    }

    pub fn get_new_skills(&self, scan_timestamp: &str) -> Result<Vec<Skill>, AppError> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT id, name, path_hash, library_path, original_source_path,
                    original_git_url, original_git_subpath, group_name, description,
                    frontmatter, created_at, mtime_ms, source_type, is_deleted
             FROM skills WHERE first_seen_at = ?1 AND is_deleted = 0
             ORDER BY name",
            )
            .map_err(|e| AppError::Config(format!("Failed to prepare new skills query: {}", e)))?;

        Self::query_skills(&mut stmt, [scan_timestamp])
    }

    pub fn get_updated_skills(&self, scan_timestamp: &str) -> Result<Vec<Skill>, AppError> {
        let mut stmt = self
            .conn
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
            .map_err(|e| {
                AppError::Config(format!("Failed to prepare updated skills query: {}", e))
            })?;

        Self::query_skills(&mut stmt, [scan_timestamp])
    }

    pub fn get_deleted_skills(&self) -> Result<Vec<Skill>, AppError> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT id, name, path_hash, library_path, original_source_path,
                    original_git_url, original_git_subpath, group_name, description,
                    frontmatter, created_at, mtime_ms, source_type, is_deleted
             FROM skills WHERE is_deleted = 1 ORDER BY last_seen_at DESC",
            )
            .map_err(|e| {
                AppError::Config(format!("Failed to prepare deleted skills query: {}", e))
            })?;

        Self::query_skills(&mut stmt, [])
    }

    pub fn get_all_active(&self) -> Result<Vec<Skill>, AppError> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT id, name, path_hash, library_path, original_source_path,
                    original_git_url, original_git_subpath, group_name, description,
                    frontmatter, created_at, mtime_ms, source_type, is_deleted
             FROM skills WHERE is_deleted = 0 ORDER BY name",
            )
            .map_err(|e| {
                AppError::Config(format!("Failed to prepare active skills query: {}", e))
            })?;

        Self::query_skills(&mut stmt, [])
    }

    pub fn get_skill_by_id(&self, id: &str) -> Result<Option<Skill>, AppError> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT id, name, path_hash, library_path, original_source_path,
                    original_git_url, original_git_subpath, group_name, description,
                    frontmatter, created_at, mtime_ms, source_type, is_deleted
             FROM skills WHERE id = ?1",
            )
            .map_err(|e| AppError::Config(format!("Failed to prepare skill query: {}", e)))?;

        let mut skills = Self::query_skills(&mut stmt, [id])?;
        Ok(skills.pop())
    }

    fn query_skills<P: rusqlite::Params>(
        stmt: &mut rusqlite::Statement,
        params: P,
    ) -> Result<Vec<Skill>, AppError> {
        let rows = stmt
            .query_map(params, |row| {
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
                    source_revision: None,
                    source_remote_revision: None,
                    source_update_status: Default::default(),
                })
            })
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

#[derive(Debug, Clone, serde::Serialize)]
pub struct ScanDiff {
    pub added: Vec<SkillWithStatus>,
    pub updated: Vec<SkillWithStatus>,
    pub deleted: Vec<SkillWithStatus>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::models::{Skill, SkillSourceType};
    use std::collections::HashMap;
    use tempfile::NamedTempFile;

    fn create_test_skill(name: &str, id: &str) -> Skill {
        Skill {
            id: id.to_string(),
            name: name.to_string(),
            path_hash: "hash1234".to_string(),
            library_path: format!("/test/{}", name),
            original_source_path: Some(format!("/source/{}", name)),
            original_git_url: None,
            original_git_subpath: None,
            group: "default".to_string(),
            description: "Test description".to_string(),
            frontmatter: HashMap::new(),
            created_at: "2024-01-01T00:00:00Z".to_string(),
            mtime_ms: 12345678,
            source_type: SkillSourceType::LocalFolder,
            is_deleted: false,
            content_hash: None,
            source_revision: None,
            source_remote_revision: None,
            source_update_status: Default::default(),
        }
    }

    #[test]
    fn test_init_schema_creates_tables() {
        let temp = NamedTempFile::new().unwrap();
        let db = ScanDatabase::new(&temp.path().to_path_buf()).unwrap();
        let count: i64 = db
            .conn
            .query_row(
                "SELECT count(*) FROM sqlite_master WHERE type='table' AND name='skills'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn test_upsert_and_get_skill() {
        let temp = NamedTempFile::new().unwrap();
        let db = ScanDatabase::new(&temp.path().to_path_buf()).unwrap();
        let skill = create_test_skill("test-skill", "skill-1");

        db.upsert_skill(&skill, "2024-01-01T00:00:00Z").unwrap();

        let retrieved = db.get_skill_by_id("skill-1").unwrap();
        assert!(retrieved.is_some());
        let retrieved = retrieved.unwrap();
        assert_eq!(retrieved.name, "test-skill");
        assert_eq!(retrieved.id, "skill-1");
    }

    #[test]
    fn test_upsert_updates_existing() {
        let temp = NamedTempFile::new().unwrap();
        let db = ScanDatabase::new(&temp.path().to_path_buf()).unwrap();
        let mut skill = create_test_skill("test-skill", "skill-1");

        db.upsert_skill(&skill, "2024-01-01T00:00:00Z").unwrap();
        skill.description = "Updated description".to_string();
        db.upsert_skill(&skill, "2024-01-02T00:00:00Z").unwrap();

        let retrieved = db.get_skill_by_id("skill-1").unwrap().unwrap();
        assert_eq!(retrieved.description, "Updated description");
        assert!(!retrieved.is_deleted);
    }

    #[test]
    fn test_mark_missing_as_deleted() {
        let temp = NamedTempFile::new().unwrap();
        let db = ScanDatabase::new(&temp.path().to_path_buf()).unwrap();
        let skill = create_test_skill("old-skill", "skill-old");

        db.upsert_skill(&skill, "2024-01-01T00:00:00Z").unwrap();
        let deleted = db.mark_missing_as_deleted("2024-01-02T00:00:00Z").unwrap();

        assert_eq!(deleted, vec!["skill-old"]);

        let active = db.get_all_active().unwrap();
        assert!(active.is_empty());

        let deleted_skills = db.get_deleted_skills().unwrap();
        assert!(deleted_skills.is_empty());
    }

    #[test]
    fn test_get_new_skills() {
        let temp = NamedTempFile::new().unwrap();
        let db = ScanDatabase::new(&temp.path().to_path_buf()).unwrap();
        let skill = create_test_skill("new-skill", "skill-new");
        let timestamp = "2024-01-01T00:00:00Z";

        db.upsert_skill(&skill, timestamp).unwrap();

        let new_skills = db.get_new_skills(timestamp).unwrap();
        assert_eq!(new_skills.len(), 1);
        assert_eq!(new_skills[0].name, "new-skill");
    }

    #[test]
    fn test_get_updated_skills() {
        let temp = NamedTempFile::new().unwrap();
        let db = ScanDatabase::new(&temp.path().to_path_buf()).unwrap();
        let mut skill = create_test_skill("update-skill", "skill-upd");

        db.upsert_skill(&skill, "2024-01-01T00:00:00Z").unwrap();
        skill.description = "Changed".to_string();
        db.upsert_skill(&skill, "2024-01-02T00:00:00Z").unwrap();

        let updated = db.get_updated_skills("2024-01-02T00:00:00Z").unwrap();
        assert_eq!(updated.len(), 1);
        assert_eq!(updated[0].description, "Changed");
    }

    #[test]
    fn test_get_all_active() {
        let temp = NamedTempFile::new().unwrap();
        let db = ScanDatabase::new(&temp.path().to_path_buf()).unwrap();
        let skill1 = create_test_skill("skill-a", "id-a");
        let skill2 = create_test_skill("skill-b", "id-b");

        db.upsert_skill(&skill1, "2024-01-01T00:00:00Z").unwrap();
        db.upsert_skill(&skill2, "2024-01-01T00:00:00Z").unwrap();

        let active = db.get_all_active().unwrap();
        assert_eq!(active.len(), 2);
    }

    #[test]
    fn test_get_skill_by_id_not_found() {
        let temp = NamedTempFile::new().unwrap();
        let db = ScanDatabase::new(&temp.path().to_path_buf()).unwrap();

        let result = db.get_skill_by_id("nonexistent").unwrap();
        assert!(result.is_none());
    }
}
