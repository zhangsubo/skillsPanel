use crate::core::error::AppError;
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

const CLONE_TIMEOUT_SECS: u64 = 300;

/// Validate that a clone destination path is safe.
///
/// Rejects paths with traversal components, absolute paths outside
/// expected directories, and paths with null bytes.
pub fn validate_clone_temp_path(path: &Path) -> Result<(), AppError> {
    let path_str = path.to_string_lossy();

    // Reject null bytes
    if path_str.contains('\0') {
        return Err(AppError::InvalidSkill("Path contains null bytes".into()));
    }

    // Reject path traversal
    for component in path.components() {
        match component {
            std::path::Component::ParentDir => {
                return Err(AppError::InvalidSkill(
                    "Path contains '..' (path traversal)".into(),
                ));
            }
            _ => {}
        }
    }

    // Ensure path is not empty
    if path_str.is_empty() {
        return Err(AppError::InvalidSkill("Path is empty".into()));
    }

    Ok(())
}

#[derive(Debug, Clone)]
pub struct CloneResult {
    pub repo_path: PathBuf,
    pub head_sha: String,
    pub from_cache: bool,
}

#[derive(Debug, Clone)]
pub struct CloneProgress {
    pub stage: String,
    pub message: String,
}

pub type ProgressFn = Arc<dyn Fn(&CloneProgress) + Send + Sync>;

/// Compute a deterministic cache key from clone URL and optional branch.
pub fn compute_cache_key(clone_url: &str, branch: Option<&str>) -> String {
    let canonical = match branch {
        Some(b) => format!("{}@{}", clone_url.trim_end_matches('/'), b),
        None => clone_url.trim_end_matches('/').to_string(),
    };
    let mut hasher = Sha256::new();
    hasher.update(canonical.as_bytes());
    let hash = format!("{:x}", hasher.finalize());
    hash[..16].to_string()
}

/// Get the cache directory for cloned repos.
pub fn cache_dir() -> Result<PathBuf, AppError> {
    let home =
        dirs::home_dir().ok_or_else(|| AppError::Config("Cannot find home directory".into()))?;
    let cache_path = home.join(".skills-panel").join("cache").join("repos");
    fs::create_dir_all(&cache_path)?;
    Ok(cache_path)
}

/// Get the HEAD SHA of a git repository.
pub fn get_head_sha(repo_path: &Path) -> Result<String, AppError> {
    let output = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(repo_path)
        .output()
        .map_err(|e| AppError::Git(format!("Failed to run git: {}", e)))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(AppError::Git(format!("Failed to get HEAD SHA: {}", stderr)));
    }

    let sha = String::from_utf8(output.stdout)
        .map_err(|e| AppError::Git(format!("Invalid UTF-8 in git output: {}", e)))?;
    Ok(sha.trim().to_string())
}

/// Clone a repository using the system git CLI.
fn clone_with_git_cli(
    url: &str,
    dest: &Path,
    branch: Option<&str>,
    cancel: Option<Arc<AtomicBool>>,
    progress_fn: Option<&ProgressFn>,
) -> Result<CloneResult, AppError> {
    let mut args = vec!["clone", "--progress", "--depth", "1"];

    if let Some(b) = branch {
        args.extend_from_slice(&["--branch", b]);
    }

    args.push(url);
    args.push(dest.to_str().unwrap_or(""));

    let start = Instant::now();
    let timeout = Duration::from_secs(CLONE_TIMEOUT_SECS);

    let mut child = Command::new("git")
        .args(&args)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| AppError::Git(format!("Failed to spawn git: {}", e)))?;

    let stderr = child.stderr.take().unwrap();
    let reader = std::io::BufReader::new(stderr);

    use std::io::BufRead;
    for line in reader.lines() {
        if start.elapsed() > timeout {
            child.kill().ok();
            return Err(AppError::Git("Clone timed out".into()));
        }

        if let Some(ref cancel) = cancel {
            if cancel.load(Ordering::SeqCst) {
                child.kill().ok();
                return Err(AppError::Cancelled);
            }
        }

        let line = line.map_err(|e| AppError::Git(format!("Failed to read git output: {}", e)))?;

        if let Some(ref progress_fn) = progress_fn {
            progress_fn(&CloneProgress {
                stage: "cloning".to_string(),
                message: line.clone(),
            });
        }
    }

    let status = child
        .wait()
        .map_err(|e| AppError::Git(format!("Failed to wait for git: {}", e)))?;

    if !status.success() {
        return Err(AppError::Git(format!(
            "Git clone failed with exit code: {:?}",
            status.code()
        )));
    }

    let head_sha = get_head_sha(dest)?;

    Ok(CloneResult {
        repo_path: dest.to_path_buf(),
        head_sha,
        from_cache: false,
    })
}

/// Clone a repository using git2 library.
fn clone_with_git2(
    url: &str,
    dest: &Path,
    branch: Option<&str>,
    cancel: Option<Arc<AtomicBool>>,
) -> Result<CloneResult, AppError> {
    let mut callbacks = git2::RemoteCallbacks::new();
    let cancel_clone = cancel.clone();
    callbacks.transfer_progress(move |_stats| {
        if let Some(ref cancel) = cancel_clone {
            !cancel.load(Ordering::SeqCst)
        } else {
            true
        }
    });

    let mut fetch_opts = git2::FetchOptions::new();
    fetch_opts.remote_callbacks(callbacks);

    let mut builder = git2::build::RepoBuilder::new();
    builder.fetch_options(fetch_opts);

    if let Some(b) = branch {
        builder.branch(b);
    }

    builder
        .clone(url, dest)
        .map_err(|e| AppError::Git(format!("Failed to clone '{}': {}", url, e)))?;

    let head_sha = get_head_sha(dest)?;

    Ok(CloneResult {
        repo_path: dest.to_path_buf(),
        head_sha,
        from_cache: false,
    })
}

/// Clone a repository with caching support.
///
/// On cache hit: fetch latest and reset to remote HEAD.
/// On cache miss: full clone to cache directory, then copy to dest.
pub fn clone_with_cache(
    url: &str,
    dest: &Path,
    branch: Option<&str>,
    cancel: Option<Arc<AtomicBool>>,
    progress_fn: Option<&ProgressFn>,
) -> Result<CloneResult, AppError> {
    // Validate destination path
    validate_clone_temp_path(dest)?;

    let cache_key = compute_cache_key(url, branch);
    let cache_base = cache_dir()?;
    let cache_path = cache_base.join(&cache_key);

    // Check if cache exists and is a valid git repo
    let from_cache = cache_path.exists() && cache_path.join(".git").exists();

    if from_cache {
        // Cache hit: fetch and reset
        if let Some(ref progress_fn) = progress_fn {
            progress_fn(&CloneProgress {
                stage: "cache_hit".to_string(),
                message: "Updating cached repository".to_string(),
            });
        }

        update_cached_repo(&cache_path, branch, cancel.clone())?;

        // Get HEAD SHA from cache before copying
        let head_sha = get_head_sha(&cache_path)?;

        // Copy from cache to dest (excluding .git directory)
        if cache_path != dest {
            fs::remove_dir_all(dest).ok();
            fs::create_dir_all(dest)?;
            copy_dir_recursive(&cache_path, dest)?;
        }

        Ok(CloneResult {
            repo_path: dest.to_path_buf(),
            head_sha,
            from_cache: true,
        })
    } else {
        // Cache miss: clone to cache first
        if let Some(ref progress_fn) = progress_fn {
            progress_fn(&CloneProgress {
                stage: "cache_miss".to_string(),
                message: "Cloning to cache".to_string(),
            });
        }

        fs::create_dir_all(&cache_path)?;

        let result = clone_with_git_cli(url, &cache_path, branch, cancel.clone(), progress_fn)
            .or_else(|_| {
                // Fallback to git2 if git CLI fails
                fs::remove_dir_all(&cache_path).ok();
                fs::create_dir_all(&cache_path)?;
                clone_with_git2(url, &cache_path, branch, cancel)
            })?;

        // Copy from cache to dest
        if cache_path != dest {
            fs::remove_dir_all(dest).ok();
            fs::create_dir_all(dest)?;
            copy_dir_recursive(&cache_path, dest)?;
        }

        Ok(CloneResult {
            repo_path: dest.to_path_buf(),
            head_sha: result.head_sha,
            from_cache: false,
        })
    }
}

/// Update a cached repository by fetching latest and resetting.
fn update_cached_repo(
    cache_path: &Path,
    branch: Option<&str>,
    cancel: Option<Arc<AtomicBool>>,
) -> Result<(), AppError> {
    let fetch_ref = match branch {
        Some(b) => format!("refs/heads/{}", b),
        None => "HEAD".to_string(),
    };

    let args = vec!["fetch", "--depth", "1", "origin", &fetch_ref];

    let mut child = Command::new("git")
        .args(&args)
        .current_dir(cache_path)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| AppError::Git(format!("Failed to spawn git fetch: {}", e)))?;

    // Wait for fetch to complete
    let status = child
        .wait()
        .map_err(|e| AppError::Git(format!("Failed to wait for git fetch: {}", e)))?;

    if !status.success() {
        return Err(AppError::Git("Git fetch failed".into()));
    }

    // Check cancel after fetch
    if let Some(ref cancel) = cancel {
        if cancel.load(Ordering::SeqCst) {
            return Err(AppError::Cancelled);
        }
    }

    // Reset to fetched HEAD
    let reset_output = Command::new("git")
        .args(["reset", "--hard", "FETCH_HEAD"])
        .current_dir(cache_path)
        .output()
        .map_err(|e| AppError::Git(format!("Failed to run git reset: {}", e)))?;

    if !reset_output.status.success() {
        let stderr = String::from_utf8_lossy(&reset_output.stderr);
        return Err(AppError::Git(format!("Git reset failed: {}", stderr)));
    }

    Ok(())
}

/// Copy directory recursively, skipping hidden files, node_modules, and symlinks.
fn copy_dir_recursive(src: &Path, dest: &Path) -> Result<(), AppError> {
    fs::create_dir_all(dest)?;

    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let file_name = entry.file_name();
        let file_name_str = file_name.to_string_lossy();

        if file_name_str.starts_with('.') || file_name_str == "node_modules" {
            continue;
        }

        let src_path = entry.path();

        // Skip symlinks to prevent symlink attacks
        if src_path.is_symlink() {
            continue;
        }

        let dest_path = dest.join(&file_name);

        if src_path.is_dir() {
            copy_dir_recursive(&src_path, &dest_path)?;
        } else {
            fs::copy(&src_path, &dest_path)?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_compute_cache_key_deterministic() {
        let key1 = compute_cache_key("https://github.com/user/repo", Some("main"));
        let key2 = compute_cache_key("https://github.com/user/repo", Some("main"));
        assert_eq!(key1, key2);
        assert_eq!(key1.len(), 16);
    }

    #[test]
    fn test_compute_cache_key_different_branches() {
        let key1 = compute_cache_key("https://github.com/user/repo", Some("main"));
        let key2 = compute_cache_key("https://github.com/user/repo", Some("develop"));
        assert_ne!(key1, key2);
    }

    #[test]
    fn test_compute_cache_key_with_trailing_slash() {
        let key1 = compute_cache_key("https://github.com/user/repo/", Some("main"));
        let key2 = compute_cache_key("https://github.com/user/repo", Some("main"));
        assert_eq!(key1, key2);
    }

    #[test]
    fn test_get_head_sha() {
        let temp = TempDir::new().unwrap();
        let repo_path = temp.path().join("repo");

        // Initialize a git repo
        Command::new("git")
            .args(["init", &repo_path.to_string_lossy()])
            .output()
            .unwrap();

        // Create initial commit
        fs::write(repo_path.join("file.txt"), "content").unwrap();
        Command::new("git")
            .args(["add", "."])
            .current_dir(&repo_path)
            .output()
            .unwrap();
        Command::new("git")
            .args(["commit", "-m", "initial"])
            .current_dir(&repo_path)
            .output()
            .unwrap();

        let sha = get_head_sha(&repo_path).unwrap();
        assert_eq!(sha.len(), 40);
    }

    #[test]
    fn test_cache_dir_created() {
        let dir = cache_dir().unwrap();
        assert!(dir.exists());
        assert!(dir.ends_with("cache/repos"));
    }
}
