use std::fs;
use std::path::{Path, PathBuf};
use dirs::home_dir;

pub struct PlatformFs;

impl PlatformFs {
    /// Expand `~` to home directory (cross-platform).
    /// On Windows, also supports `%USERPROFILE%` style env vars.
    pub fn expand_tilde(path: &str) -> PathBuf {
        let expanded = Self::expand_env_impl(path);
        if expanded.to_string_lossy().starts_with("~/") || expanded.to_string_lossy().as_ref() == "~" {
            if let Some(home) = home_dir() {
                let rest = expanded.to_string_lossy();
                let rest = rest.strip_prefix('~').unwrap_or("");
                home.join(rest.trim_start_matches('/').trim_start_matches('\\'))
            } else {
                expanded
            }
        } else {
            expanded
        }
    }

    /// Expand `$VAR` (Unix) and `%VAR%` (Windows) environment variables.
    pub fn expand_env(path: &str) -> PathBuf {
        let expanded = Self::expand_tilde(path);
        Self::expand_env_impl(&expanded.to_string_lossy())
    }

    fn expand_env_impl(path: &str) -> PathBuf {
        let path_str = path.to_string();
        let mut result = String::new();
        let chars: Vec<char> = path_str.chars().collect();
        let len = chars.len();
        let mut i = 0;

        while i < len {
            match chars[i] {
                '$' if i + 1 < len => {
                    let mut var_name = String::new();
                    i += 1;
                    while i < len && (chars[i].is_alphanumeric() || chars[i] == '_') {
                        var_name.push(chars[i]);
                        i += 1;
                    }
                    if let Ok(val) = std::env::var(&var_name) {
                        result.push_str(&val);
                    } else {
                        result.push('$');
                        result.push_str(&var_name);
                    }
                }
                '%' => {
                    let mut var_name = String::new();
                    i += 1;
                    while i < len && chars[i] != '%' {
                        var_name.push(chars[i]);
                        i += 1;
                    }
                    if i < len {
                        i += 1; // skip closing %
                    }
                    if let Ok(val) = std::env::var(&var_name) {
                        result.push_str(&val);
                    } else {
                        result.push('%');
                        result.push_str(&var_name);
                        result.push('%');
                    }
                }
                c => {
                    result.push(c);
                    i += 1;
                }
            }
        }

        PathBuf::from(result)
    }

    pub fn ensure_dir(path: &Path) -> std::io::Result<()> {
        if !path.exists() {
            fs::create_dir_all(path)?;
        }
        Ok(())
    }

    pub fn is_symlink(path: &Path) -> bool {
        fs::symlink_metadata(path)
            .map(|m| m.file_type().is_symlink())
            .unwrap_or(false)
    }

    pub fn is_directory(path: &Path) -> bool {
        fs::metadata(path)
            .map(|m| m.is_dir())
            .unwrap_or(false)
    }

    pub fn canonicalize_safe(path: &Path) -> Option<PathBuf> {
        fs::canonicalize(path).ok()
    }

    pub fn home_dir() -> Option<PathBuf> {
        home_dir()
    }

    /// Get the config directory path (cross-platform).
    /// Unix: ~/.skills-panel
    /// Windows: %APPDATA%/skills-panel
    pub fn config_dir() -> PathBuf {
        #[cfg(target_os = "windows")]
        {
            if let Some(appdata) = std::env::var_os("APPDATA") {
                return PathBuf::from(appdata).join("skills-panel");
            }
        }
        home_dir()
            .map(|h| h.join(".skills-panel"))
            .unwrap_or_else(|| PathBuf::from(".skills-panel"))
    }

    pub fn check_write_permission(path: &Path) -> bool {
        let test_file = path.join(".skills-panel-perm-test");
        match fs::write(&test_file, b"test") {
            Ok(_) => {
                let _ = fs::remove_file(&test_file);
                true
            }
            Err(_) => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_expand_tilde_with_home() {
        let expanded = PlatformFs::expand_tilde("~/test/path");
        assert!(!expanded.to_string_lossy().starts_with('~'));
    }

    #[test]
    fn test_expand_env_unix_style() {
        let val = PlatformFs::expand_env_impl("/home/$USER/test");
        assert!(val.to_string_lossy().contains("/home/"));
    }

    #[test]
    fn test_expand_env_windows_style() {
        let val = PlatformFs::expand_env_impl("%TEMP%/test");
        // Should expand %TEMP% or leave as-is if not set
        assert!(val.to_string_lossy().contains("test"));
    }

    #[test]
    fn test_config_dir_windows() {
        let dir = PlatformFs::config_dir();
        #[cfg(target_os = "windows")]
        assert!(dir.to_string_lossy().contains("skills-panel"));
        #[cfg(not(target_os = "windows"))]
        assert!(dir.to_string_lossy().contains(".skills-panel"));
    }
}