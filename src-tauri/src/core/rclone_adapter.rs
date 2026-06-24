use crate::core::error::AppError;
use crate::core::models::{
    RcloneProgress, SyncActionType, SyncDirection, SyncPlan, SyncPlanAction, SyncPlanStats,
    SyncProviderKind, SyncResult, SyncStatus,
};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;

const RCLONE_DOWNLOAD_BASE: &str = "https://downloads.rclone.org";

pub struct RcloneAdapter {
    binary_path: PathBuf,
    config_dir: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteConfig {
    pub remote_name: String,
    pub kind: SyncProviderKind,
    pub url: String,
    pub username: String,
    pub password: String,
    #[serde(default)]
    pub extra_params: std::collections::HashMap<String, String>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct RcloneListEntry {
    path: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    is_dir: Option<bool>,
    #[serde(default)]
    size: Option<i64>,
    #[serde(default)]
    mod_time: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RcloneJsonLog {
    #[serde(default)]
    level: Option<String>,
    #[serde(default)]
    msg: Option<String>,
    #[serde(default)]
    stats: Option<RcloneStatsObj>,
    #[serde(default)]
    object: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RcloneStatsObj {
    #[serde(default)]
    bytes: Option<i64>,
    #[serde(default)]
    total_bytes: Option<i64>,
    #[serde(default)]
    speed: Option<i64>,
    #[serde(default)]
    transfers: Option<i64>,
}

impl RcloneAdapter {
    pub fn new(config_dir: &Path) -> Result<Self, AppError> {
        let binary_path = Self::find_binary(config_dir)?;
        Ok(Self {
            binary_path,
            config_dir: config_dir.to_path_buf(),
        })
    }

    pub fn binary_path(&self) -> &Path {
        &self.binary_path
    }

    pub fn local_bin_dir(config_dir: &Path) -> PathBuf {
        config_dir.join("bin")
    }

    pub fn find_available(config_dir: &Path) -> Option<PathBuf> {
        let local = Self::local_bin_dir(config_dir).join(Self::binary_name());
        if local.is_file() {
            return Some(local);
        }
        which::which("rclone").ok()
    }

    fn binary_name() -> String {
        if cfg!(target_os = "windows") {
            "rclone.exe".into()
        } else {
            "rclone".into()
        }
    }

    fn platform_download_info() -> (&'static str, &'static str, &'static str) {
        let os = match std::env::consts::OS {
            "macos" => "darwin",
            "linux" => "linux",
            "windows" => "windows",
            other => other,
        };
        let arch = match std::env::consts::ARCH {
            "x86_64" => "amd64",
            "aarch64" => "arm64",
            other => other,
        };
        let ext = if cfg!(target_os = "windows") {
            "zip"
        } else {
            "tar.gz"
        };
        (os, arch, ext)
    }

    pub fn download_url() -> String {
        let (os, arch, ext) = Self::platform_download_info();
        format!(
            "{}/rclone-current-{}-{}.{}",
            RCLONE_DOWNLOAD_BASE, os, arch, ext
        )
    }

    pub async fn download_rclone(config_dir: &Path) -> Result<PathBuf, AppError> {
        let bin_dir = Self::local_bin_dir(config_dir);
        fs::create_dir_all(&bin_dir).map_err(|e| {
            AppError::Download(format!("Failed to create bin directory: {}", e))
        })?;

        let url = Self::download_url();
        let (_, _, ext) = Self::platform_download_info();

        let temp_dir = tempfile::tempdir().map_err(|e| {
            AppError::Download(format!("Failed to create temp directory: {}", e))
        })?;
        let archive_path = temp_dir.path().join(format!("rclone.{}", ext));

        let response = reqwest::get(&url).await?;
        if !response.status().is_success() {
            return Err(AppError::Download(format!(
                "Failed to download rclone: HTTP {}",
                response.status()
            )));
        }

        let bytes = response.bytes().await?;
        fs::write(&archive_path, &bytes).map_err(|e| {
            AppError::Download(format!("Failed to write archive: {}", e))
        })?;

        if ext == "zip" {
            Self::extract_zip(&archive_path, &bin_dir)?;
        } else {
            Self::extract_tar_gz(&archive_path, &bin_dir)?;
        }

        let binary_path = bin_dir.join(Self::binary_name());
        if !binary_path.is_file() {
            Self::find_and_move_binary(&bin_dir)?;
        }

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let binary_path = bin_dir.join(Self::binary_name());
            if binary_path.is_file() {
                fs::set_permissions(&binary_path, fs::Permissions::from_mode(0o755))
                    .map_err(|e| {
                        AppError::Download(format!("Failed to set executable permissions: {}", e))
                    })?;
            }
        }

        let binary_path = bin_dir.join(Self::binary_name());
        if binary_path.is_file() {
            Ok(binary_path)
        } else {
            Err(AppError::Download(
                "rclone binary not found after extraction".into(),
            ))
        }
    }

    fn extract_zip(archive_path: &Path, dest: &Path) -> Result<(), AppError> {
        let file = fs::File::open(archive_path).map_err(|e| {
            AppError::Download(format!("Failed to open zip archive: {}", e))
        })?;
        let mut archive = zip::ZipArchive::new(file).map_err(|e| {
            AppError::Download(format!("Failed to read zip archive: {}", e))
        })?;

        for i in 0..archive.len() {
            let mut entry = archive.by_index(i).map_err(|e| {
                AppError::Download(format!("Failed to read zip entry: {}", e))
            })?;
            let out_path = match entry.enclosed_name() {
                Some(path) => dest.join(path),
                None => continue,
            };

            if entry.is_dir() {
                fs::create_dir_all(&out_path).map_err(|e| {
                    AppError::Download(format!("Failed to create directory: {}", e))
                })?;
            } else {
                if let Some(parent) = out_path.parent() {
                    fs::create_dir_all(parent).map_err(|e| {
                        AppError::Download(format!("Failed to create parent directory: {}", e))
                    })?;
                }
                let mut out_file = fs::File::create(&out_path).map_err(|e| {
                    AppError::Download(format!("Failed to create file: {}", e))
                })?;
                std::io::copy(&mut entry, &mut out_file).map_err(|e| {
                    AppError::Download(format!("Failed to extract file: {}", e))
                })?;
            }
        }

        Ok(())
    }

    fn extract_tar_gz(archive_path: &Path, dest: &Path) -> Result<(), AppError> {
        let file = fs::File::open(archive_path).map_err(|e| {
            AppError::Download(format!("Failed to open tar.gz archive: {}", e))
        })?;
        let gz_decoder = flate2::read::GzDecoder::new(file);
        let mut archive = tar::Archive::new(gz_decoder);

        archive.unpack(dest).map_err(|e| {
            AppError::Download(format!("Failed to extract tar.gz archive: {}", e))
        })?;

        Ok(())
    }

    fn find_and_move_binary(bin_dir: &Path) -> Result<(), AppError> {
        let binary_name = Self::binary_name();
        for entry in fs::read_dir(bin_dir).map_err(|e| {
            AppError::Download(format!("Failed to read bin directory: {}", e))
        })? {
            let entry = entry.map_err(|e| {
                AppError::Download(format!("Failed to read directory entry: {}", e))
            })?;
            let path = entry.path();
            if path.is_dir() {
                let candidate = path.join(&binary_name);
                if candidate.is_file() {
                    let dest = bin_dir.join(&binary_name);
                    fs::rename(&candidate, &dest).map_err(|e| {
                        AppError::Download(format!("Failed to move rclone binary: {}", e))
                    })?;
                    let _ = fs::remove_dir(&path);
                    return Ok(());
                }
                if let Ok(mut sub_entries) = fs::read_dir(&path) {
                    while let Some(sub_entry) = sub_entries.next() {
                        let sub_entry = sub_entry.map_err(|e| {
                            AppError::Download(format!("Failed to read sub-entry: {}", e))
                        })?;
                        let sub_path = sub_entry.path();
                        if sub_path.file_name() == Some(std::ffi::OsStr::new(&binary_name))
                            && sub_path.is_file()
                        {
                            let dest = bin_dir.join(&binary_name);
                            fs::rename(&sub_path, &dest).map_err(|e| {
                                AppError::Download(format!("Failed to move rclone binary: {}", e))
                            })?;
                            let _ = fs::remove_dir(&path);
                            return Ok(());
                        }
                    }
                }
            }
        }
        Ok(())
    }

    fn find_binary(config_dir: &Path) -> Result<PathBuf, AppError> {
        let local = Self::local_bin_dir(config_dir).join(Self::binary_name());
        if local.is_file() {
            return Ok(local);
        }
        which::which("rclone").map_err(|_| {
            AppError::Sync(
                "rclone not found. Please install rclone or use the download button.".into(),
            )
        })
    }

    fn base_args(&self) -> Vec<String> {
        vec![
            "--config".into(),
            self.config_dir
                .join("rclone.conf")
                .to_string_lossy()
                .into_owned(),
            "--use-json-log".into(),
        ]
    }

    async fn run(&self, args: &[String]) -> Result<String, AppError> {
        let mut all_args = self.base_args();
        all_args.extend_from_slice(args);

        let output = Command::new(&self.binary_path)
            .args(&all_args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await
            .map_err(|e| AppError::Sync(format!("Failed to execute rclone: {}", e)))?;

        let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
        let stderr = String::from_utf8_lossy(&output.stderr).into_owned();

        if !output.status.success() {
            return Err(AppError::Sync(format!(
                "rclone exited with code {:?}: {}",
                output.status.code(),
                if stderr.is_empty() { &stdout } else { &stderr }
            )));
        }

        Ok(stdout)
    }

    pub async fn test_connection(&self, remote_name: &str) -> Result<(), AppError> {
        let remote_path = format!("{}:", remote_name);
        self.run(&[
            "lsd".into(),
            remote_path,
            "--max-depth".into(),
            "1".into(),
        ])
        .await?;
        Ok(())
    }

    pub async fn create_remote(&self, config: &RemoteConfig) -> Result<(), AppError> {
        let kind_str = match config.kind {
            SyncProviderKind::WebDav => "webdav",
            SyncProviderKind::S3 => "s3",
            SyncProviderKind::Sftp => "sftp",
        };

        let mut args = vec![
            "config".into(),
            "create".into(),
            config.remote_name.clone(),
            kind_str.into(),
        ];

        match config.kind {
            SyncProviderKind::WebDav => {
                args.push("url".into());
                args.push(config.url.clone());
                args.push("user".into());
                args.push(config.username.clone());
                args.push("pass".into());
                // rclone obscures passwords; use standard input
                args.push(config.password.clone());
            }
            SyncProviderKind::S3 => {
                args.push("endpoint".into());
                args.push(config.url.clone());
                args.push("access_key_id".into());
                args.push(config.username.clone());
                args.push("secret_access_key".into());
                args.push(config.password.clone());
            }
            SyncProviderKind::Sftp => {
                args.push("host".into());
                args.push(config.url.clone());
                args.push("user".into());
                args.push(config.username.clone());
                args.push("pass".into());
                args.push(config.password.clone());
            }
        }

        for (k, v) in &config.extra_params {
            args.push(k.clone());
            args.push(v.clone());
        }

        // --non-interactive so it doesn't prompt
        args.push("--non-interactive".into());

        self.run(&args).await?;
        Ok(())
    }

    pub async fn delete_remote(&self, remote_name: &str) -> Result<(), AppError> {
        self.run(&[
            "config".into(),
            "delete".into(),
            remote_name.into(),
            "--non-interactive".into(),
        ])
        .await?;
        Ok(())
    }

    /// Ensure the remote root directory exists.
    pub async fn ensure_remote_dir(&self, remote_name: &str, sub_path: &str) -> Result<(), AppError> {
        let remote_path = format!("{}:{}", remote_name, sub_path);
        let result = self
            .run(&["mkdir".into(), remote_path.clone()])
            .await;
        match result {
            Ok(_) => Ok(()),
            Err(AppError::Sync(msg)) if msg.contains("405") || msg.contains("409") => {
                // Directory already exists
                Ok(())
            }
            Err(e) => Err(e),
        }
    }

    /// Run bidirectional sync (bisync) — the core sync method.
    pub async fn bisync(
        &self,
        remote_name: &str,
        local_path: &Path,
        sub_path: &str,
        on_progress: impl Fn(RcloneProgress) + Send + 'static,
    ) -> Result<SyncResult, AppError> {
        let remote_path = format!("{}:{}", remote_name, sub_path);

        self.ensure_remote_dir(remote_name, sub_path).await?;

        let local = local_path.to_string_lossy().into_owned();
        let mut args = self.base_args();
        args.extend_from_slice(&[
            "bisync".into(),
            local.clone(),
            remote_path,
            "--resilient".into(),
            "--conflict-suffix".into(),
            "conflict".into(),
            "--force".into(),
            "--check-sync".into(),
            "true".into(),
        ]);

        let started_at = chrono::Utc::now().to_rfc3339();
        let mut bytes_transferred: i64 = 0;
        let mut skills_count: i64 = 0;
        let mut errors: Vec<String> = Vec::new();

        let mut child = Command::new(&self.binary_path)
            .args(&args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| AppError::Sync(format!("Failed to spawn rclone: {}", e)))?;

        // Parse JSON log from stderr (rclone --use-json-log writes to stderr)
        if let Some(stderr) = child.stderr.take() {
            let reader = BufReader::new(stderr);
            let mut lines = reader.lines();
            while let Ok(Some(line)) = lines.next_line().await {
                if let Ok(log) = serde_json::from_str::<RcloneJsonLog>(&line) {
                    if let Some(stats) = log.stats {
                        bytes_transferred = stats.bytes.unwrap_or(0);
                        skills_count = stats.transfers.unwrap_or(0);
                        on_progress(RcloneProgress {
                            bytes_transferred,
                            bytes_total: stats.total_bytes.unwrap_or(0),
                            percentage: if stats.total_bytes.unwrap_or(0) > 0 {
                                (bytes_transferred as f64 / stats.total_bytes.unwrap_or(1) as f64)
                                    * 100.0
                            } else {
                                0.0
                            },
                            speed: stats.speed.unwrap_or(0),
                            current_file: log.object.unwrap_or_default(),
                        });
                    }
                    if log.level.as_deref() == Some("error") {
                        errors.push(log.msg.unwrap_or_default());
                    }
                }
            }
        }

        let status = child.wait().await.map_err(|e| {
            AppError::Sync(format!("Failed to wait for rclone: {}", e))
        })?;

        let finished_at = chrono::Utc::now().to_rfc3339();
        let sync_status = if status.success() {
            SyncStatus::Success
        } else if errors.is_empty() {
            SyncStatus::Failed
        } else {
            SyncStatus::Failed
        };

        Ok(SyncResult {
            provider_id: remote_name.to_string(),
            direction: SyncDirection::Bisync,
            status: sync_status,
            started_at,
            finished_at: Some(finished_at),
            bytes_transferred,
            skills_synced: skills_count,
            errors,
        })
    }

    /// Run one-way sync (upload local → remote).
    pub async fn sync_to_remote(
        &self,
        remote_name: &str,
        local_path: &Path,
        sub_path: &str,
    ) -> Result<SyncResult, AppError> {
        let remote_path = format!("{}:{}", remote_name, sub_path);
        let local = local_path.to_string_lossy().into_owned();

        let started_at = chrono::Utc::now().to_rfc3339();

        self.run(&["sync".into(), local, remote_path]).await?;

        let finished_at = chrono::Utc::now().to_rfc3339();

        Ok(SyncResult {
            provider_id: remote_name.to_string(),
            direction: SyncDirection::Upload,
            status: SyncStatus::Success,
            started_at,
            finished_at: Some(finished_at),
            bytes_transferred: 0,
            skills_synced: 0,
            errors: vec![],
        })
    }

    /// Run one-way sync (download remote → local).
    pub async fn sync_from_remote(
        &self,
        remote_name: &str,
        local_path: &Path,
        sub_path: &str,
    ) -> Result<SyncResult, AppError> {
        let remote_path = format!("{}:{}", remote_name, sub_path);
        let local = local_path.to_string_lossy().into_owned();

        let started_at = chrono::Utc::now().to_rfc3339();

        self.run(&["sync".into(), remote_path, local]).await?;

        let finished_at = chrono::Utc::now().to_rfc3339();

        Ok(SyncResult {
            provider_id: remote_name.to_string(),
            direction: SyncDirection::Download,
            status: SyncStatus::Success,
            started_at,
            finished_at: Some(finished_at),
            bytes_transferred: 0,
            skills_synced: 0,
            errors: vec![],
        })
    }

    /// Check differences between local and remote (dry-run).
    pub async fn check(
        &self,
        remote_name: &str,
        local_path: &Path,
        sub_path: &str,
    ) -> Result<SyncPlan, AppError> {
        let remote_path = format!("{}:{}", remote_name, sub_path);
        let local = local_path.to_string_lossy().into_owned();

        let output = self
            .run(&[
                "check".into(),
                local.clone(),
                remote_path,
                "--differ".into(),
                "--missing-on-dst".into(),
                "--missing-on-src".into(),
            ])
            .await;

        let mut actions: Vec<SyncPlanAction> = Vec::new();
        let mut stats = SyncPlanStats {
            upload_count: 0,
            download_count: 0,
            conflict_count: 0,
            delete_local_count: 0,
            delete_remote_count: 0,
            skip_count: 0,
        };

        // Parse rclone check output
        match output {
            Ok(check_output) => {
                // rclone check outputs lines like:
                // "2024/01/01 12:00:00 ERROR : file.txt: differences found"
                // "file.txt: not found in dst"
                // "file.txt: not found in src"
                // "file.txt: hash differs"
                for line in check_output.lines() {
                    let trimmed = line.trim();
                    if trimmed.contains("not found in dst") || trimmed.contains("missing on dst") {
                        if let Some(name) = extract_filename(trimmed) {
                            actions.push(SyncPlanAction {
                                skill_name: name,
                                action_type: SyncActionType::UploadLocal,
                                local_mtime: None,
                                remote_mtime: None,
                                reason: "Local only".into(),
                            });
                            stats.upload_count += 1;
                        }
                    } else if trimmed.contains("not found in src")
                        || trimmed.contains("missing on src")
                    {
                        if let Some(name) = extract_filename(trimmed) {
                            actions.push(SyncPlanAction {
                                skill_name: name,
                                action_type: SyncActionType::DownloadRemote,
                                local_mtime: None,
                                remote_mtime: None,
                                reason: "Remote only".into(),
                            });
                            stats.download_count += 1;
                        }
                    } else if trimmed.contains("differ") || trimmed.contains("hash differs") {
                        if let Some(name) = extract_filename(trimmed) {
                            actions.push(SyncPlanAction {
                                skill_name: name,
                                action_type: SyncActionType::Conflict,
                                local_mtime: None,
                                remote_mtime: None,
                                reason: "Both modified".into(),
                            });
                            stats.conflict_count += 1;
                        }
                    }
                }
            }
            Err(AppError::Sync(msg)) => {
                // rclone check returns exit code 1 when differences found
                if !msg.contains("exit") || !msg.contains("differences") {
                    // Real error
                    return Err(AppError::Sync(msg));
                }
            }
            Err(e) => return Err(e),
        }

        Ok(SyncPlan {
            provider_id: remote_name.to_string(),
            provider_name: remote_name.to_string(),
            actions,
            stats,
        })
    }
}

fn extract_filename(line: &str) -> Option<String> {
    // rclone check output: "ERROR : filename: not found in dst"
    // Extract the filename between ": " and the next ":"
    let parts: Vec<&str> = line.splitn(3, ": ").collect();
    if parts.len() >= 2 {
        let name = parts[1].trim();
        if !name.is_empty() {
            return Some(name.to_string());
        }
    }
    None
}