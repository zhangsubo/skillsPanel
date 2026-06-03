//! SyncEngine — orchestrates the full sync flow: history row start, build
//! or download the archive, drive a `SyncProvider`, record finish +
//! audit, update provider's last_sync_* fields.
//!
//! The engine is transport-agnostic. The concrete provider
//! (GitHub / WebDAV / future) is injected by the command layer.

use crate::core::database::{
    AuditRepository, Database, SyncHistoryRepository, SyncProvidersRepository,
};
use crate::core::error::AppError;
use crate::core::models::SyncHistory;
use crate::core::sync::archive;
use crate::core::sync::provider::{SyncDirection, SyncProvider};
use std::path::PathBuf;
use std::sync::Arc;

/// Cancellation marker: the engine uses a unique payload
/// (`AppError::Config("__sync_cancelled__")`) so it can distinguish a
/// user cancel from a real error without string-matching arbitrary
/// provider messages.
fn cancelled_error() -> AppError {
    AppError::Config("__sync_cancelled__".into())
}

fn is_cancelled(e: &AppError) -> bool {
    matches!(e, AppError::Config(msg) if msg == "__sync_cancelled__")
}

/// Config key holding the user-supplied archive password.
/// Stored encrypted via SENSITIVE_KEYS.
pub const PASSWORD_CONFIG_KEY: &str = "backup_archive_password";

/// Filename prefix for new archives. Format:
/// `{prefix}_{YYYYMMDD-HHMMSS}.zip.enc`
pub const ARCHIVE_FILENAME_PREFIX: &str = "skills_panel_backup";

pub struct SyncOutcome {
    pub history: SyncHistory,
    pub staged_path: Option<PathBuf>,
}

impl std::fmt::Debug for SyncOutcome {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SyncOutcome")
            .field("history", &self.history)
            .field("staged_path", &self.staged_path)
            .finish()
    }
}

/// Higher-level entry: given the provider's DB id, fetch the record, build
/// the concrete provider (caller-injected factory), and run sync.
pub fn run_sync_by_id<P, F>(
    conn: &Database,
    provider_id: &str,
    direction: SyncDirection,
    archive_password: &str,
    library_path: &PathBuf,
    factory: F,
) -> Result<SyncOutcome, AppError>
where
    P: SyncProvider,
    F: FnOnce(&crate::core::models::SyncProvider) -> Result<P, AppError>,
{
    if archive_password.is_empty() {
        return Err(AppError::Config(
            "Backup password is not configured. Set it in Settings → Backup.".into(),
        ));
    }

    let prov_repo = SyncProvidersRepository::new(conn);
    let provider = prov_repo
        .get(provider_id)?
        .ok_or_else(|| AppError::Config(format!("Provider {provider_id} not found")))?;
    let concrete = factory(&provider)?;

    let hist_repo = SyncHistoryRepository::new(conn);
    let history_id = hist_repo.record_start(&provider.id, direction.as_str())?;

    let mut outcome = SyncOutcome {
        history: SyncHistory {
            id: history_id.clone(),
            provider_id: provider.id.clone(),
            direction: direction.as_str().to_string(),
            status: "in_progress".to_string(),
            started_at: chrono::Utc::now().to_rfc3339(),
            finished_at: None,
            bytes_transferred: None,
            skills_count: None,
            error_message: None,
        },
        staged_path: None,
    };

    let mut bytes_transferred: i64 = 0;
    let mut skills_count: i64 = 0;

    let result: Result<(), AppError> = (|| {
        concrete.prepare_remote()?;
        match direction {
            SyncDirection::Upload => {
                let archive = archive::build_archive(conn, archive_password)?;
                skills_count = archive.skills_count as i64;
                bytes_transferred = archive.bytes.len() as i64;
                let filename = format!(
                    "{}_{}.zip.enc",
                    ARCHIVE_FILENAME_PREFIX,
                    chrono::Utc::now().format("%Y%m%d-%H%M%S")
                );
                concrete.upload(&archive.bytes, &filename)?;
            }
            SyncDirection::Download => {
                let (bytes, _name) = concrete.download_latest()?;
                bytes_transferred = bytes.len() as i64;
                let stage_dir = crate::core::sync::provider::archive_cache_dir(library_path);
                let staged = stage_dir.join(format!(
                    "downloaded_{}.zip.enc",
                    chrono::Utc::now().format("%Y%m%d-%H%M%S")
                ));
                std::fs::write(&staged, &bytes)?;
                outcome.staged_path = Some(staged.clone());

                let extract_dir = library_path.join(".sync_staging");
                std::fs::create_dir_all(&extract_dir)?;
                let manifest = archive::extract_archive(&bytes, archive_password, &extract_dir)?;
                skills_count = manifest.skills.len() as i64;
            }
        }
        Ok(())
    })();

    match result {
        Ok(()) => {
            hist_repo.finish(
                &history_id,
                "success",
                Some(bytes_transferred),
                Some(skills_count),
                None,
            )?;
            prov_repo.record_last_sync(&provider.id, "success", None)?;
            AuditRepository::new(conn).log(
                &format!("sync_{}", direction.as_str()),
                &provider.id,
                Some(format!("skills={skills_count} bytes={bytes_transferred}")),
                true,
                None,
            )?;
        }
        Err(e) => {
            let cancelled = is_cancelled(&e);
            let status = if cancelled { "cancelled" } else { "error" };
            let msg = if cancelled { String::new() } else { e.to_string() };
            let bytes_to_record = if cancelled { None } else { Some(bytes_transferred) };
            let skills_to_record = if cancelled { None } else { Some(skills_count) };
            let _ = hist_repo.finish(
                &history_id,
                status,
                bytes_to_record,
                skills_to_record,
                if cancelled { None } else { Some(msg.as_str()) },
            );
            let _ = prov_repo.record_last_sync(
                &provider.id,
                status,
                if cancelled { None } else { Some(msg.as_str()) },
            );
            if !cancelled {
                let _ = AuditRepository::new(conn).log(
                    &format!("sync_{}", direction.as_str()),
                    &provider.id,
                    Some(msg.clone()),
                    false,
                    Some(msg.clone()),
                );
            }
            return Err(e);
        }
    }

    let final_history = SyncHistoryRepository::new(conn)
        .list_for_provider(&provider.id, 1)?
        .into_iter()
        .find(|h| h.id == history_id)
        .unwrap_or(outcome.history.clone());
    outcome.history = final_history;
    Ok(outcome)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::database::{Database, SkillsRepository, SyncProvidersRepository};
    use crate::core::models::{Skill, SkillSourceType};
    use crate::core::sync::provider::RemoteFile;
    use std::collections::HashMap;
    use std::sync::Mutex;
    use tempfile::TempDir;

    fn make_db() -> Database {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        Database::new(&tmp.path().to_path_buf()).unwrap()
    }

    fn seed_skill(db: &Database, id: &str, name: &str, dir: &std::path::Path) {
        let skill = Skill {
            id: id.to_string(),
            name: name.to_string(),
            path_hash: "h".into(),
            library_path: dir.to_string_lossy().to_string(),
            original_source_path: None,
            original_git_url: None,
            original_git_subpath: None,
            group: "default".into(),
            description: "".into(),
            frontmatter: HashMap::new(),
            created_at: "2024-01-01T00:00:00Z".into(),
            mtime_ms: 0,
            source_type: SkillSourceType::LocalFolder,
            is_deleted: false,
            content_hash: None,
            source_revision: None,
            source_remote_revision: None,
            source_update_status: crate::core::models::SourceUpdateStatus::Unknown,
        };
        SkillsRepository::new(db).upsert(&skill).unwrap();
    }

    struct MockProvider {
        upload_count: Arc<Mutex<usize>>,
        fail_next: bool,
    }

    impl MockProvider {
        fn new() -> Self {
            Self {
                upload_count: Arc::new(Mutex::new(0)),
                fail_next: false,
            }
        }
    }

    impl SyncProvider for MockProvider {
        fn kind(&self) -> &'static str {
            "mock"
        }
        fn upload(&self, _bytes: &[u8], _filename: &str) -> Result<(), AppError> {
            if self.fail_next {
                return Err(AppError::Config("mock upload failure".into()));
            }
            *self.upload_count.lock().unwrap() += 1;
            Ok(())
        }
        fn download_latest(&self) -> Result<(Vec<u8>, String), AppError> {
            Ok((vec![], "x.zip.enc".into()))
        }
        fn list_remote(&self) -> Result<Vec<RemoteFile>, AppError> {
            Ok(vec![])
        }
        fn test_connection(&self) -> Result<(), AppError> {
            Ok(())
        }
    }

    #[test]
    fn test_engine_upload_writes_history_and_calls_provider() {
        let db = make_db();
        let lib = TempDir::new().unwrap();
        let skill_dir = lib.path().join("alpha");
        std::fs::create_dir_all(&skill_dir).unwrap();
        std::fs::write(skill_dir.join("SKILL.md"), "# alpha").unwrap();
        seed_skill(&db, "s1", "alpha", &skill_dir);

        let prov = SyncProvidersRepository::new(&db)
            .create("p1", "mock", "{}", true)
            .unwrap();
        let mock = MockProvider::new();
        let upload_count = mock.upload_count.clone();

        let outcome = run_sync_by_id(
            &db,
            &prov.id,
            SyncDirection::Upload,
            "secret",
            &lib.path().to_path_buf(),
            |_| Ok(mock),
        )
        .unwrap();

        assert_eq!(outcome.history.status, "success");
        assert_eq!(*upload_count.lock().unwrap(), 1);
        assert!(outcome.history.bytes_transferred.unwrap() > 0);
        assert_eq!(outcome.history.skills_count, Some(1));
    }

    #[test]
    fn test_engine_records_history_failure_and_propagates_error() {
        let db = make_db();
        let lib = TempDir::new().unwrap();
        let prov = SyncProvidersRepository::new(&db)
            .create("p1", "mock", "{}", true)
            .unwrap();
        let mut mock = MockProvider::new();
        mock.fail_next = true;

        let err = run_sync_by_id(
            &db,
            &prov.id,
            SyncDirection::Upload,
            "secret",
            &lib.path().to_path_buf(),
            |_| Ok(mock),
        )
        .unwrap_err();
        assert!(err.to_string().contains("mock upload failure"), "got: {err}");

        let recent = SyncHistoryRepository::new(&db).list_recent(1).unwrap();
        assert_eq!(recent[0].status, "error");
        assert!(recent[0].error_message.is_some());
    }

    #[test]
    fn test_engine_rejects_empty_password() {
        let db = make_db();
        let lib = TempDir::new().unwrap();
        let prov = SyncProvidersRepository::new(&db)
            .create("p1", "mock", "{}", true)
            .unwrap();
        let err = run_sync_by_id(
            &db,
            &prov.id,
            SyncDirection::Upload,
            "",
            &lib.path().to_path_buf(),
            |_| Ok(MockProvider::new()),
        )
        .unwrap_err();
        assert!(err.to_string().contains("password"), "got: {err}");
    }

    #[test]
    fn test_engine_unknown_provider_id_errors() {
        let db = make_db();
        let lib = TempDir::new().unwrap();
        let err = run_sync_by_id(
            &db,
            "nope",
            SyncDirection::Upload,
            "secret",
            &lib.path().to_path_buf(),
            |_| Ok(MockProvider::new()),
        )
        .unwrap_err();
        assert!(err.to_string().contains("not found"), "got: {err}");
    }

    #[test]
    fn test_engine_factory_error_propagates() {
        let db = make_db();
        let lib = TempDir::new().unwrap();
        let prov = SyncProvidersRepository::new(&db)
            .create("p1", "mock", "{}", true)
            .unwrap();
        let err = run_sync_by_id(
            &db,
            &prov.id,
            SyncDirection::Upload,
            "secret",
            &lib.path().to_path_buf(),
            |_| Err::<MockProvider, _>(AppError::Config("factory failed".into())),
        )
        .unwrap_err();
        assert!(err.to_string().contains("factory failed"), "got: {err}");
    }
}
