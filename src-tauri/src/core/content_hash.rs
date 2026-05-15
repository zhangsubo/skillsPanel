use crate::core::error::AppError;
use crate::core::fs_utils;
use std::path::Path;

pub struct ContentHash;

impl ContentHash {
    pub fn hash_directory(dir: &Path) -> Result<String, AppError> {
        fs_utils::hash_directory(dir)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_hash_directory_basic() {
        let temp = TempDir::new().unwrap();
        let dir = temp.path().join("skill");
        fs::create_dir(&dir).unwrap();
        fs::write(dir.join("SKILL.md"), "---\nname: test\n---\n# Body").unwrap();
        fs::write(dir.join("body.md"), "Hello world").unwrap();

        let hash = ContentHash::hash_directory(&dir).unwrap();
        assert_eq!(hash.len(), 64);
    }

    #[test]
    fn test_hash_directory_deterministic() {
        let temp = TempDir::new().unwrap();
        let dir = temp.path().join("skill");
        fs::create_dir(&dir).unwrap();
        fs::write(dir.join("file.txt"), "content").unwrap();

        let hash1 = ContentHash::hash_directory(&dir).unwrap();
        let hash2 = ContentHash::hash_directory(&dir).unwrap();
        assert_eq!(hash1, hash2);
    }

    #[test]
    fn test_hash_directory_different_content() {
        let temp = TempDir::new().unwrap();

        let dir1 = temp.path().join("skill1");
        fs::create_dir(&dir1).unwrap();
        fs::write(dir1.join("file.txt"), "version1").unwrap();

        let dir2 = temp.path().join("skill2");
        fs::create_dir(&dir2).unwrap();
        fs::write(dir2.join("file.txt"), "version2").unwrap();

        let hash1 = ContentHash::hash_directory(&dir1).unwrap();
        let hash2 = ContentHash::hash_directory(&dir2).unwrap();
        assert_ne!(hash1, hash2);
    }

    #[test]
    fn test_hash_directory_nonexistent() {
        let result = ContentHash::hash_directory(Path::new("/nonexistent/path"));
        assert!(result.is_err());
    }
}
