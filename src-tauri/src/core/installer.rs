use crate::core::error::AppError;
use crate::core::fs_utils;
use crate::core::library::SkillLibrary;
use std::fs;
use std::path::{Path, PathBuf};

pub struct Installer;

impl Installer {
    pub fn install_from_local(source_path: &Path, name: &str, library: &SkillLibrary) -> Result<PathBuf, AppError> {
        if !source_path.exists() {
            return Err(AppError::InstallFailed(format!("Source path does not exist: {}", source_path.display())));
        }

        let skill_md = source_path.join("SKILL.md");
        if !skill_md.exists() {
            return Err(AppError::InvalidSkill(format!("No SKILL.md found in {}", source_path.display())));
        }

        let content = fs::read_to_string(&skill_md)?;
        let (frontmatter, _) = fs_utils::parse_frontmatter(&content)
            .ok_or_else(|| AppError::InvalidSkill("Invalid YAML frontmatter".into()))?;

        if name.is_empty() && frontmatter.get("name").is_none() {
            return Err(AppError::Validation("Skill name is required when SKILL.md has no 'name' field".into()));
        }

        let final_name = if name.is_empty() {
            frontmatter.get("name").and_then(|v| v.as_str()).unwrap().to_string()
        } else {
            name.to_string()
        };

        if library.skill_exists(&final_name) {
            return Err(AppError::Conflict("该项目已经安装,无需重复安装".to_string()));
        }

        let dest = library.add_skill(source_path, &final_name)?;
        Ok(dest)
    }

    pub fn install_from_local_zip(zip_path: &Path, skill_root: &Path, name: &str, library: &SkillLibrary) -> Result<PathBuf, AppError> {
        let temp_dir = tempfile::tempdir()?;
        let extracted = fs_utils::extract_zip(zip_path, temp_dir.path())?;

        let skill_dir = extracted.join(skill_root);
        if !skill_dir.exists() {
            return Err(AppError::InstallFailed(format!(
                "Extracted skill directory not found: {}", skill_root.display()
            )));
        }

        Self::install_from_local(&skill_dir, name, library)
    }

    pub fn install_with_overwrite(source_path: &Path, name: &str, library: &SkillLibrary) -> Result<PathBuf, AppError> {
        let dest = library.add_skill_with_overwrite(source_path, name)?;
        Ok(dest)
    }
}
