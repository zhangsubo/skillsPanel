use crate::core::error::AppError;
use fs2::FileExt;
use std::fs::{self, File};

pub struct RepoLock {
    _guard: File,
}

impl RepoLock {
    pub fn acquire(description: &str) -> Result<Self, AppError> {
        let lock_dir = dirs::home_dir()
            .ok_or_else(|| AppError::Config("Cannot find home directory".into()))?
            .join(".skills-panel");

        fs::create_dir_all(&lock_dir)
            .map_err(|e| AppError::Config(format!("Failed to create lock directory: {}", e)))?;

        let lock_path = lock_dir.join(".install.lock");
        let file = File::create(&lock_path)
            .map_err(|e| AppError::Config(format!("Failed to create lock file: {}", e)))?;

        file.lock_exclusive().map_err(|e| {
            AppError::Config(format!("Failed to acquire lock ({description}): {e}"))
        })?;

        Ok(Self { _guard: file })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_acquire_and_drop() {
        let lock = RepoLock::acquire("test").unwrap();
        drop(lock);
    }
}
