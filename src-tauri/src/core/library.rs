use crate::core::error::AppError;
use crate::core::config::AppConfig;
use crate::core::fs_utils;
use sha2::{Sha256, Digest};
use std::fs;
use std::path::{Path, PathBuf};

pub struct SkillLibrary {
    library_path: PathBuf,
}

impl SkillLibrary {
    pub fn new(config: &AppConfig) -> Result<Self, AppError> {
        fs::create_dir_all(&config.library_path)?;
        Ok(Self { library_path: config.library_path.clone() })
    }

    pub fn library_path(&self) -> &Path {
        &self.library_path
    }

    pub fn skill_path(&self, name: &str) -> PathBuf {
        self.library_path.join(name)
    }

    pub fn compute_path_hash(path: &Path) -> String {
        let path_str = path.to_string_lossy();
        let mut hasher = Sha256::new();
        hasher.update(path_str.as_bytes());
        format!("{:x}", hasher.finalize())[..8].to_string()
    }

    pub fn compute_skill_id(name: &str, path: &Path) -> String {
        let hash = Self::compute_path_hash(path);
        format!("{}-{}", name, hash)
    }

    pub fn add_skill(&self, source_path: &Path, name: &str) -> Result<PathBuf, AppError> {
        let dest = self.skill_path(name);
        if dest.exists() {
            return Err(AppError::Conflict(format!(
                "Skill '{}' already exists in library at {}", name, dest.display()
            )));
        }
        self.copy_skill_dir(source_path, &dest)?;
        Ok(dest)
    }

    pub fn add_skill_with_overwrite(&self, source_path: &Path, name: &str) -> Result<PathBuf, AppError> {
        let dest = self.skill_path(name);
        if dest.exists() {
            fs::remove_dir_all(&dest)?;
        }
        self.copy_skill_dir(source_path, &dest)?;
        Ok(dest)
    }

    pub fn remove_skill(&self, name: &str) -> Result<(), AppError> {
        let path = self.skill_path(name);
        if path.exists() {
            fs::remove_dir_all(&path)?;
        }
        Ok(())
    }

    pub fn rename_skill(&self, old_name: &str, new_name: &str) -> Result<PathBuf, AppError> {
        let old_path = self.skill_path(old_name);
        let new_path = self.skill_path(new_name);
        if !old_path.exists() {
            return Err(AppError::SkillNotFound(old_name.into()));
        }
        if new_path.exists() {
            return Err(AppError::Conflict(format!("Skill '{}' already exists", new_name)));
        }
        fs::rename(&old_path, &new_path)?;
        Ok(new_path)
    }

    pub fn list_skills(&self) -> Result<Vec<String>, AppError> {
        let mut skills = Vec::new();
        if !self.library_path.exists() {
            return Ok(skills);
        }
        for entry in fs::read_dir(&self.library_path)? {
            let entry = entry?;
            if entry.file_type()?.is_dir() {
                let skill_dir = entry.path();
                if skill_dir.join("SKILL.md").exists() {
                    if let Some(name) = entry.file_name().to_str() {
                        skills.push(name.to_string());
                    }
                }
            }
        }
        skills.sort();
        Ok(skills)
    }

    pub fn skill_exists(&self, name: &str) -> bool {
        self.skill_path(name).join("SKILL.md").exists()
    }

    fn copy_skill_dir(&self, src: &Path, dest: &Path) -> Result<(), AppError> {
        fs::create_dir_all(dest)?;
        fs_utils::copy_dir_recursive(src, dest)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;
    use crate::core::config::AppConfig;

    fn create_test_config(library_path: &Path) -> AppConfig {
        AppConfig {
            library_path: library_path.join("library"),
            tools: vec![],
            sources: vec![],
            sync: crate::core::models::SyncConfig { mode: crate::core::models::SyncMode::Symlink },
            install: crate::core::models::InstallConfig { allow_zip: true, allow_git: true, default_sync_targets: vec![] },
            exclude_paths: vec![],
            rules: crate::core::models::RulesConfig::default(),
            deleted_skills: vec![],
        }
    }

    #[test]
    fn test_compute_path_hash_consistency() {
        let path = Path::new("/test/skill");
        let hash1 = SkillLibrary::compute_path_hash(path);
        let hash2 = SkillLibrary::compute_path_hash(path);
        assert_eq!(hash1, hash2);
        assert_eq!(hash1.len(), 8);
    }

    #[test]
    fn test_compute_skill_id_format() {
        let path = Path::new("/test/skill");
        let id = SkillLibrary::compute_skill_id("my-skill", path);
        assert!(id.starts_with("my-skill-"));
        assert!(id.len() > "my-skill-".len());
    }

    #[test]
    fn test_skill_path() {
        let temp = TempDir::new().unwrap();
        let config = create_test_config(temp.path());
        let lib = SkillLibrary::new(&config).unwrap();
        assert_eq!(lib.skill_path("foo"), temp.path().join("library").join("foo"));
    }

    #[test]
    fn test_add_and_list_skills() {
        let temp = TempDir::new().unwrap();
        let config = create_test_config(temp.path());
        let lib = SkillLibrary::new(&config).unwrap();

        let src = temp.path().join("source-skill");
        fs::create_dir(&src).unwrap();
        fs::write(src.join("SKILL.md"), "---\nname: source-skill\n---").unwrap();

        let dest = lib.add_skill(&src, "source-skill").unwrap();
        assert!(dest.exists());

        let skills = lib.list_skills().unwrap();
        assert_eq!(skills, vec!["source-skill"]);
    }

    #[test]
    fn test_add_skill_conflict() {
        let temp = TempDir::new().unwrap();
        let config = create_test_config(temp.path());
        let lib = SkillLibrary::new(&config).unwrap();

        let src = temp.path().join("source-skill");
        fs::create_dir(&src).unwrap();
        fs::write(src.join("SKILL.md"), "---\nname: source-skill\n---").unwrap();

        lib.add_skill(&src, "source-skill").unwrap();
        let result = lib.add_skill(&src, "source-skill");
        assert!(result.is_err());
    }

    #[test]
    fn test_remove_skill() {
        let temp = TempDir::new().unwrap();
        let config = create_test_config(temp.path());
        let lib = SkillLibrary::new(&config).unwrap();

        let src = temp.path().join("to-remove");
        fs::create_dir(&src).unwrap();
        fs::write(src.join("SKILL.md"), "---\nname: to-remove\n---").unwrap();

        lib.add_skill(&src, "to-remove").unwrap();
        assert!(lib.skill_exists("to-remove"));

        lib.remove_skill("to-remove").unwrap();
        assert!(!lib.skill_exists("to-remove"));
    }

    #[test]
    fn test_rename_skill() {
        let temp = TempDir::new().unwrap();
        let config = create_test_config(temp.path());
        let lib = SkillLibrary::new(&config).unwrap();

        let src = temp.path().join("old-name");
        fs::create_dir(&src).unwrap();
        fs::write(src.join("SKILL.md"), "---\nname: old-name\n---").unwrap();

        lib.add_skill(&src, "old-name").unwrap();
        let new_path = lib.rename_skill("old-name", "new-name").unwrap();
        assert!(new_path.exists());
        assert!(!lib.skill_exists("old-name"));
        assert!(lib.skill_exists("new-name"));
    }

    #[test]
    fn test_rename_skill_not_found() {
        let temp = TempDir::new().unwrap();
        let config = create_test_config(temp.path());
        let lib = SkillLibrary::new(&config).unwrap();

        let result = lib.rename_skill("missing", "new");
        assert!(result.is_err());
    }

    #[test]
    fn test_list_skills_empty_library() {
        let temp = TempDir::new().unwrap();
        let config = create_test_config(temp.path());
        let lib = SkillLibrary::new(&config).unwrap();

        let skills = lib.list_skills().unwrap();
        assert!(skills.is_empty());
    }

    #[test]
    fn test_add_skill_with_overwrite() {
        let temp = TempDir::new().unwrap();
        let config = create_test_config(temp.path());
        let lib = SkillLibrary::new(&config).unwrap();

        let src1 = temp.path().join("source1");
        fs::create_dir(&src1).unwrap();
        fs::write(src1.join("SKILL.md"), "---\nname: overwrite\n---\nv1").unwrap();

        let src2 = temp.path().join("source2");
        fs::create_dir(&src2).unwrap();
        fs::write(src2.join("SKILL.md"), "---\nname: overwrite\n---\nv2").unwrap();

        lib.add_skill(&src1, "overwrite").unwrap();
        lib.add_skill_with_overwrite(&src2, "overwrite").unwrap();

        let content = fs::read_to_string(lib.skill_path("overwrite").join("SKILL.md")).unwrap();
        assert!(content.contains("v2"));
    }
}
