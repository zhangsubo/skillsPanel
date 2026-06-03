//! Cloud sync module: archive, providers, and the sync engine.
//!
//! All exports here are additive; the install path is untouched.

use crate::core::error::AppError;

pub mod archive;
pub mod engine;
pub mod github_zip;
pub mod provider;
pub mod webdav;

pub use archive::{build_archive, extract_archive, EncryptedArchive, ARCHIVE_MAGIC};
pub use engine::{run_sync_by_id, SyncOutcome, ARCHIVE_FILENAME_PREFIX, PASSWORD_CONFIG_KEY};
pub use github_zip::GitHubZipProvider;
pub use provider::{
    archive_cache_dir, parse_config_json, ConfigField, ProviderInfo, RemoteFile, SyncDirection,
    SyncProvider,
};
pub use webdav::WebDavProvider;

/// Newtype wrapper so a `Box<dyn SyncProvider>` can be used as a generic
/// `P: SyncProvider` in `run_sync_by_id`. The wrapper just delegates
/// every method to the inner trait object.
pub struct BoxedSyncProvider(Box<dyn SyncProvider>);

impl BoxedSyncProvider {
    pub fn new<P: SyncProvider + 'static>(p: P) -> Self {
        Self(Box::new(p))
    }
}

impl SyncProvider for BoxedSyncProvider {
    fn kind(&self) -> &'static str {
        <dyn SyncProvider>::kind(&*self.0)
    }
    fn upload(&self, bytes: &[u8], filename: &str) -> Result<(), AppError> {
        <dyn SyncProvider>::upload(&*self.0, bytes, filename)
    }
    fn download_latest(&self) -> Result<(Vec<u8>, String), AppError> {
        <dyn SyncProvider>::download_latest(&*self.0)
    }
    fn list_remote(&self) -> Result<Vec<RemoteFile>, AppError> {
        <dyn SyncProvider>::list_remote(&*self.0)
    }
    fn test_connection(&self) -> Result<(), AppError> {
        <dyn SyncProvider>::test_connection(&*self.0)
    }
    fn prepare_remote(&self) -> Result<(), AppError> {
        <dyn SyncProvider>::prepare_remote(&*self.0)
    }
}
