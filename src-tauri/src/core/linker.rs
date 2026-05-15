use crate::core::error::AppError;
use crate::core::fs_utils;
use crate::core::models::SkillToolStatus;
use std::fs;
use std::path::Path;

/// Create a symbolic link, cross-platform.
/// On Unix: uses std::os::unix::fs::symlink.
/// On Windows: uses std::os::windows::fs::symlink_dir/symlink_file.
/// Falls back to copy mode if symlink creation fails on Windows.
#[cfg(target_os = "windows")]
fn create_symlink(src: &Path, dst: &Path) -> Result<(), AppError> {
    use std::os::windows::fs as windows_fs;
    let result = if src.is_dir() {
        windows_fs::symlink_dir(src, dst)
    } else {
        windows_fs::symlink_file(src, dst)
    };
    match result {
        Ok(()) => Ok(()),
        Err(e) => {
            let msg = format!(
                "Failed to create symlink on Windows: {}. \
                 Try enabling Developer Mode (Settings → Update & Security → For developers) \
                 or running as Administrator. Falling back to copy mode.",
                e
            );
            Err(AppError::Link(msg))
        }
    }
}

#[cfg(not(target_os = "windows"))]
fn create_symlink(src: &Path, dst: &Path) -> Result<(), AppError> {
    std::os::unix::fs::symlink(src, dst)
        .map_err(|e| AppError::Link(format!("Failed to create symlink: {}", e)))
}

pub struct Linker;

impl Linker {
    pub fn link(skill_path: &Path, tool_dir: &Path, skill_name: &str) -> Result<(), AppError> {
        let target = tool_dir.join(skill_name);
        Self::ensure_tool_dir(tool_dir)?;

        if target.exists() || target.is_symlink() {
            let stat = fs::symlink_metadata(&target)?;
            if stat.is_symlink() {
                let link_dest = fs::read_link(&target)?;
                let resolved = Self::resolve_link(&target, &link_dest);
                if resolved == skill_path.canonicalize()? {
                    return Ok(());
                }
                fs::remove_file(&target)?;
            } else if stat.is_dir() {
                return Err(AppError::Link(format!(
                    "Target '{}' is a real directory, not a symlink. Remove it manually first.",
                    target.display()
                )));
            } else {
                fs::remove_file(&target)?;
            }
        }

        create_symlink(skill_path, &target)?;

        Ok(())
    }

    pub fn unlink(tool_dir: &Path, skill_name: &str) -> Result<(), AppError> {
        let target = tool_dir.join(skill_name);
        if !target.exists() && !target.is_symlink() {
            return Ok(());
        }

        let stat = fs::symlink_metadata(&target)?;
        if stat.is_symlink() {
            fs::remove_file(&target)?;
            Ok(())
        } else if stat.is_dir() {
            Err(AppError::Link(format!(
                "Target '{}' is a real directory, not a symlink. Cannot remove automatically.",
                target.display()
            )))
        } else {
            Err(AppError::Link(format!(
                "Target '{}' is a regular file, not a symlink. Cannot remove automatically.",
                target.display()
            )))
        }
    }

    pub fn fix_link(skill_path: &Path, tool_dir: &Path, skill_name: &str) -> Result<(), AppError> {
        let target = tool_dir.join(skill_name);
        if target.exists() && target.is_symlink() {
            fs::remove_file(&target)?;
        }
        Self::link(skill_path, tool_dir, skill_name)
    }

    pub fn check_status(skill_library_path: &Path, tool_dir: &Path, skill_name: &str) -> SkillToolStatus {
        let target = tool_dir.join(skill_name);

        if !target.exists() && !target.is_symlink() {
            return SkillToolStatus::Missing;
        }

        let stat = match fs::symlink_metadata(&target) {
            Ok(s) => s,
            Err(_) => return SkillToolStatus::Error,
        };

        if stat.is_symlink() {
            let link_dest = match fs::read_link(&target) {
                Ok(p) => p,
                Err(_) => return SkillToolStatus::Wrong,
            };
            let resolved = Self::resolve_link(&target, &link_dest);
            let canonical_skill = skill_library_path.canonicalize().ok();

            match canonical_skill {
                Some(canonical) => {
                    if resolved == canonical {
                        SkillToolStatus::Linked
                    } else {
                        SkillToolStatus::Wrong
                    }
                }
                None => SkillToolStatus::Error,
            }
        } else if stat.is_dir() {
            SkillToolStatus::Directory
        } else {
            SkillToolStatus::Error
        }
    }

    pub fn clean_stale(tool_dir: &Path) -> Result<Vec<String>, AppError> {
        let mut cleaned = Vec::new();
        if !tool_dir.exists() {
            return Ok(cleaned);
        }

        for entry in fs::read_dir(tool_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_symlink() {
                let link_dest = fs::read_link(&path)?;
                let resolved = Self::resolve_link(&path, &link_dest);
                if !resolved.exists() {
                    let name = entry.file_name().to_string_lossy().into_owned();
                    fs::remove_file(&path)?;
                    cleaned.push(name);
                }
            }
        }
        Ok(cleaned)
    }

    fn ensure_tool_dir(tool_dir: &Path) -> Result<(), AppError> {
        if !tool_dir.exists() {
            fs::create_dir_all(tool_dir)?;
        }
        Ok(())
    }

    fn resolve_link(link_path: &Path, link_dest: &Path) -> std::path::PathBuf {
        if link_dest.is_absolute() {
            link_dest.to_path_buf()
        } else {
            link_path.parent().unwrap_or(Path::new(".")).join(link_dest)
        }
    }

    pub fn copy_skill(source: &Path, dest: &Path) -> Result<(), AppError> {
        if dest.exists() {
            fs::remove_dir_all(dest)?;
        }
        fs::create_dir_all(dest)?;
        fs_utils::copy_dir_for_linker(source, dest)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_resolve_link_absolute() {
        let abs = Path::new("/usr/local/bin/skill");
        let resolved = Linker::resolve_link(Path::new("/tmp/link"), abs);
        assert_eq!(resolved, abs);
    }

    #[test]
    fn test_resolve_link_relative() {
        let link_path = Path::new("/home/user/.cursor/skills/my-skill");
        let link_dest = Path::new("../../.skills-panel/skills/my-skill");
        let resolved = Linker::resolve_link(link_path, link_dest);
        assert_eq!(resolved, Path::new("/home/user/.cursor/skills/../../.skills-panel/skills/my-skill"));
    }

    #[test]
    fn test_check_status_missing() {
        let temp = TempDir::new().unwrap();
        let skill_path = temp.path().join("skill");
        let tool_dir = temp.path().join("tools");
        fs::create_dir(&tool_dir).unwrap();

        let status = Linker::check_status(&skill_path, &tool_dir, "missing-skill");
        assert!(matches!(status, SkillToolStatus::Missing));
    }

    #[test]
    fn test_check_status_directory() {
        let temp = TempDir::new().unwrap();
        let skill_path = temp.path().join("skill");
        let tool_dir = temp.path().join("tools");
        fs::create_dir(&tool_dir).unwrap();
        fs::create_dir(tool_dir.join("real-dir")).unwrap();

        let status = Linker::check_status(&skill_path, &tool_dir, "real-dir");
        assert!(matches!(status, SkillToolStatus::Directory));
    }

    #[test]
    fn test_copy_dir_recursive() {
        let temp = TempDir::new().unwrap();
        let src = temp.path().join("source");
        let dest = temp.path().join("dest");
        fs::create_dir(&src).unwrap();
        fs::create_dir(&dest).unwrap();
        fs::write(src.join("file.txt"), "hello").unwrap();
        fs::create_dir(src.join("subdir")).unwrap();
        fs::write(src.join("subdir/nested.txt"), "world").unwrap();

        fs_utils::copy_dir_for_linker(&src, &dest).unwrap();

        assert!(dest.join("file.txt").exists());
        assert_eq!(fs::read_to_string(dest.join("file.txt")).unwrap(), "hello");
        assert!(dest.join("subdir/nested.txt").exists());
        assert_eq!(fs::read_to_string(dest.join("subdir/nested.txt")).unwrap(), "world");
    }

    #[test]
    fn test_copy_dir_recursive_skips_hidden() {
        let temp = TempDir::new().unwrap();
        let src = temp.path().join("source");
        let dest = temp.path().join("dest");
        fs::create_dir(&src).unwrap();
        fs::create_dir(&dest).unwrap();
        fs::write(src.join(".hidden"), "secret").unwrap();
        fs::write(src.join("visible"), "hello").unwrap();

        fs_utils::copy_dir_for_linker(&src, &dest).unwrap();

        assert!(!dest.join(".hidden").exists());
        assert!(dest.join("visible").exists());
    }

    #[test]
    fn test_copy_skill() {
        let temp = TempDir::new().unwrap();
        let src = temp.path().join("skill-src");
        let dest = temp.path().join("skill-dst");
        fs::create_dir(&src).unwrap();
        fs::write(src.join("SKILL.md"), "# Skill").unwrap();

        Linker::copy_skill(&src, &dest).unwrap();

        assert!(dest.exists());
        assert!(dest.join("SKILL.md").exists());
    }

    #[cfg(unix)]
    #[test]
    fn test_clean_stale() {
        let temp = TempDir::new().unwrap();
        let tool_dir = temp.path().join("tools");
        fs::create_dir(&tool_dir).unwrap();

        let stale_link = tool_dir.join("stale");
        std::os::unix::fs::symlink(Path::new("/nonexistent/path"), &stale_link).unwrap();

        let cleaned = Linker::clean_stale(&tool_dir).unwrap();
        assert_eq!(cleaned, vec!["stale"]);
        assert!(!stale_link.exists());
    }

    #[cfg(unix)]
    #[test]
    fn test_clean_stale_no_stale() {
        let temp = TempDir::new().unwrap();
        let tool_dir = temp.path().join("tools");
        fs::create_dir(&tool_dir).unwrap();

        let valid_target = temp.path().join("real-skill");
        fs::create_dir(&valid_target).unwrap();
        let valid_link = tool_dir.join("valid");
        std::os::unix::fs::symlink(&valid_target, &valid_link).unwrap();

        let cleaned = Linker::clean_stale(&tool_dir).unwrap();
        assert!(cleaned.is_empty());
        assert!(valid_link.exists());
    }
}
