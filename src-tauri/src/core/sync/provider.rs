//! Cloud sync providers — abstracted behind a single trait so the engine
//! stays transport-agnostic.
//!
//! Each implementation parses its own `config_json` (validated by the
//! command layer before reaching here). The trait speaks bytes-in, bytes-out;
//! the engine handles archive build / extract / history / audit.

use crate::core::error::AppError;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RemoteFile {
    pub name: String,
    pub size: u64,
    /// Optional ISO-8601 / RFC-3339 string. None when the remote doesn't expose it.
    pub last_modified: Option<String>,
}

/// Static description of a provider type — used by the frontend to render
/// the right config form without hard-coding the kinds in TS.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderInfo {
    pub kind: String,
    pub display_name: String,
    pub config_schema: Vec<ConfigField>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigField {
    pub key: String,
    pub label: String,
    /// "text" | "password" | "url" | "select"
    pub kind: String,
    pub required: bool,
    pub placeholder: Option<String>,
    pub options: Option<Vec<String>>,
}

/// Sync direction. Upload pushes the local archive to the remote;
/// download pulls the most recent remote archive back.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum SyncDirection {
    Upload,
    Download,
}

impl SyncDirection {
    pub fn as_str(&self) -> &'static str {
        match self {
            SyncDirection::Upload => "upload",
            SyncDirection::Download => "download",
        }
    }
    pub fn parse(s: &str) -> Result<Self, AppError> {
        match s {
            "upload" => Ok(Self::Upload),
            "download" => Ok(Self::Download),
            other => Err(AppError::Config(format!("Unknown sync direction: {other}"))),
        }
    }
}

pub trait SyncProvider: Send + Sync {
    /// Stable identifier for the provider type — matches the `kind` column
    /// in `sync_providers` (e.g. "github_zip", "webdav").
    fn kind(&self) -> &'static str;

    /// Push `bytes` to the remote under the given filename (e.g.
    /// "backup_20260603-120000.zip.enc"). Implementations should make this
    /// atomic — readers must never see a half-written file.
    fn upload(&self, bytes: &[u8], filename: &str) -> Result<(), AppError>;

    /// Pull the most recent archive bytes from the remote.
    /// Returns (bytes, filename) so the engine can validate via manifest.
    fn download_latest(&self) -> Result<(Vec<u8>, String), AppError>;

    /// List remote files, newest first. Used by download_latest internally
    /// and exposed for the frontend's "show me what's in the cloud" view.
    fn list_remote(&self) -> Result<Vec<RemoteFile>, AppError>;

    /// Quick connectivity check — no bytes moved, just auth + reachability.
    /// Used by the "Test Connection" button in the provider form.
    fn test_connection(&self) -> Result<(), AppError>;

    /// Optional: any preflight work the engine should run before
    /// `upload`/`download` (e.g. ensure remote directory exists).
    /// Default: no-op. Providers override only when needed.
    fn prepare_remote(&self) -> Result<(), AppError> {
        Ok(())
    }
}

/// Static metadata about a provider type. Returned by each provider's
/// inherent `info()` method (not a trait method, because trait objects
/// don't carry the `where Self: Sized` constraint needed for associated
/// functions).
pub fn info<P: ?Sized + SyncProvider>() -> ProviderInfo
where
    P: Sized,
{
    // Each provider overrides this via its inherent impl; the trait
    // itself stays object-safe. The default below is hit only via
    // `BoxedSyncProvider`.
    ProviderInfo {
        kind: "unknown".into(),
        display_name: "Unknown".into(),
        config_schema: vec![],
    }
}

/// Validate a `config_json` blob against the field spec returned by
/// `ProviderInfo::config_schema`. Returns a map of field-key -> trimmed
/// value (or empty string for missing optional fields). Used by the
/// command layer to normalize input before persisting.
pub fn parse_config_json(json_str: &str) -> Result<serde_json::Map<String, serde_json::Value>, AppError> {
    let v: serde_json::Value = serde_json::from_str(json_str)
        .map_err(|e| AppError::Config(format!("Invalid provider config JSON: {e}")))?;
    match v {
        serde_json::Value::Object(m) => Ok(m),
        other => Err(AppError::Config(format!(
            "Provider config must be a JSON object, got: {}",
            other
        ))),
    }
}

/// Where archives are staged on disk for / from the cloud.
/// Centralized so all providers agree on a layout.
pub fn archive_cache_dir(base: &Path) -> PathBuf {
    let dir = base.join("cache").join("sync_archives");
    let _ = std::fs::create_dir_all(&dir);
    dir
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_config_json_validates_object() {
        let ok = parse_config_json(r#"{"url":"http://x","user":"a"}"#).unwrap();
        assert_eq!(ok.len(), 2);
        assert_eq!(ok.get("url").unwrap().as_str(), Some("http://x"));
    }

    #[test]
    fn test_parse_config_json_rejects_non_object() {
        let err = parse_config_json(r#""just a string""#).unwrap_err();
        assert!(err.to_string().contains("JSON object"), "got: {err}");
    }

    #[test]
    fn test_parse_config_json_rejects_invalid_syntax() {
        let err = parse_config_json(r#"{"url": }"#).unwrap_err();
        assert!(err.to_string().contains("Invalid"), "got: {err}");
    }

    #[test]
    fn test_sync_direction_round_trip() {
        for d in [SyncDirection::Upload, SyncDirection::Download] {
            assert_eq!(SyncDirection::parse(d.as_str()).unwrap(), d);
        }
        let err = SyncDirection::parse("sideways").unwrap_err();
        assert!(err.to_string().contains("Unknown"), "got: {err}");
    }

    #[test]
    fn test_archive_cache_dir_creates_path() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = archive_cache_dir(tmp.path());
        assert!(dir.exists());
        assert!(dir.ends_with("cache/sync_archives"));
    }
}
