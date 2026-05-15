use crate::core::error::AppError;
use dirs::home_dir;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

pub fn expand_tilde(path: &str) -> PathBuf {
    // First expand %VAR% (Windows) and $VAR (Unix) environment variables
    let env_expanded = expand_env_vars(path);
    let expanded = env_expanded.to_string_lossy().into_owned();

    if expanded.starts_with("~/") || expanded == "~" {
        if let Some(home) = home_dir() {
            let rest = expanded.strip_prefix('~').unwrap_or("");
            home.join(rest.trim_start_matches('/').trim_start_matches('\\'))
        } else {
            PathBuf::from(&expanded)
        }
    } else {
        PathBuf::from(&expanded)
    }
}

fn expand_env_vars(path: &str) -> PathBuf {
    let mut result = String::new();
    let chars: Vec<char> = path.chars().collect();
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
                    i += 1;
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

pub fn contract_tilde(path: &str) -> String {
    if let Some(home) = home_dir() {
        let home_str = home.to_string_lossy();
        if path.starts_with(home_str.as_ref()) {
            return format!("~{}", &path[home_str.len()..]);
        }
    }
    path.to_string()
}

pub fn parse_frontmatter(content: &str) -> Option<(HashMap<String, serde_json::Value>, String)> {
    let trimmed = content.trim();
    if !trimmed.starts_with("---") {
        return None;
    }
    let rest = trimmed.strip_prefix("---")?;
    let end = rest.find("---")?;
    let yaml_str = &rest[..end];
    let body = rest[end + 3..].trim().to_string();

    let frontmatter: HashMap<String, serde_json::Value> = serde_yaml::from_str(yaml_str)
        .ok()
        .unwrap_or_default();

    Some((frontmatter, body))
}

pub fn copy_dir_recursive(src: &Path, dest: &Path) -> Result<(), AppError> {
    fs::create_dir_all(dest)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let file_name = entry.file_name();
        let file_name_str = file_name.to_string_lossy();

        if file_name_str.starts_with('.') || file_name_str == "node_modules" {
            continue;
        }

        let src_path = entry.path();
        let dest_path = dest.join(&file_name);

        if src_path.is_dir() {
            fs::create_dir_all(&dest_path)?;
            copy_dir_recursive(&src_path, &dest_path)?;
        } else {
            let src_canonical = src_path.canonicalize().ok();
            let dest_canonical = dest_path.canonicalize().ok();
            if src_canonical != dest_canonical {
                fs::copy(&src_path, &dest_path)?;
            }
        }
    }
    Ok(())
}

pub fn copy_dir_for_linker(src: &Path, dest: &Path) -> Result<(), AppError> {
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let file_name = entry.file_name();
        let file_name_str = file_name.to_string_lossy();

        if file_name_str.starts_with('.') || file_name_str == "node_modules" {
            continue;
        }

        let src_path = entry.path();
        let dest_path = dest.join(&file_name);

        if src_path.is_dir() {
            fs::create_dir_all(&dest_path)?;
            copy_dir_for_linker(&src_path, &dest_path)?;
        } else {
            if let Some(parent) = dest_path.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::copy(&src_path, &dest_path)?;
        }
    }
    Ok(())
}

pub fn extract_zip(zip_path: &Path, dest: &Path) -> Result<PathBuf, AppError> {
    let file = fs::File::open(zip_path)?;
    let mut archive = zip::ZipArchive::new(file)
        .map_err(|e| AppError::Zip(format!("Failed to open zip: {}", e)))?;

    for i in 0..archive.len() {
        let mut file = archive
            .by_index(i)
            .map_err(|e| AppError::Zip(format!("Failed to read zip entry: {}", e)))?;
        let outpath = match file.enclosed_name() {
            Some(path) => dest.join(path),
            None => continue,
        };
        if file.is_dir() {
            fs::create_dir_all(&outpath)?;
        } else {
            if let Some(parent) = outpath.parent() {
                fs::create_dir_all(parent)?;
            }
            let mut outfile = fs::File::create(&outpath)?;
            std::io::copy(&mut file, &mut outfile)?;
        }
    }
    Ok(dest.to_path_buf())
}

pub fn is_zip_file(path: &Path) -> bool {
    if path
        .extension()
        .map(|ext| ext.eq_ignore_ascii_case("zip"))
        .unwrap_or(false)
    {
        return true;
    }

    let mut file = match fs::File::open(path) {
        Ok(file) => file,
        Err(_) => return false,
    };

    let mut magic = [0u8; 4];
    if std::io::Read::read_exact(&mut file, &mut magic).is_err() {
        return false;
    }

    matches!(
        magic,
        [0x50, 0x4B, 0x03, 0x04] | [0x50, 0x4B, 0x05, 0x06] | [0x50, 0x4B, 0x07, 0x08]
    )
}

pub fn hash_directory(dir: &Path) -> Result<String, AppError> {
    if !dir.exists() {
        return Err(AppError::InvalidSkill(format!(
            "Directory does not exist: {}",
            dir.display()
        )));
    }

    let mut hasher = Sha256::new();
    let mut entries: Vec<_> = WalkDir::new(dir)
        .into_iter()
        .filter_entry(|e| {
            let name = e.file_name().to_string_lossy();
            !name.starts_with('.') && name != "node_modules" && name != ".git"
        })
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .collect();

    entries.sort_by_key(|e| e.path().to_path_buf());

    for entry in entries {
        let content = fs::read(entry.path())?;
        hasher.update(&content);
    }

    Ok(format!("{:x}", hasher.finalize()))
}

pub fn find_skill_dirs(root: &Path, exclude: &[String], recursive: bool) -> Vec<PathBuf> {
    let mut results = Vec::new();
    let max_depth = if recursive { 5 } else { 1 };

    for entry in WalkDir::new(root)
        .max_depth(max_depth)
        .into_iter()
        .filter_entry(|e| {
            let name = e.file_name().to_string_lossy();
            if name.starts_with('.') {
                return false;
            }
            if exclude.iter().any(|exc| name == *exc) {
                return false;
            }
            true
        })
        .filter_map(|e| e.ok())
    {
        if entry.file_type().is_dir() {
            let skill_md = entry.path().join("SKILL.md");
            if skill_md.exists() {
                results.push(entry.path().to_path_buf());
            }
        }
    }
    results
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    #[test]
    fn test_expand_tilde() {
        let expanded = expand_tilde("~/test/path");
        assert!(!expanded.to_string_lossy().starts_with('~'));
    }

    #[test]
    fn test_expand_tilde_no_tilde() {
        let expanded = expand_tilde("/absolute/path");
        assert_eq!(expanded, PathBuf::from("/absolute/path"));
    }

    #[test]
    fn test_contract_tilde() {
        if let Some(home) = home_dir() {
            let home_str = home.to_string_lossy();
            let path = format!("{}/test", home_str);
            let contracted = contract_tilde(&path);
            assert_eq!(contracted, "~/test");
        }
    }

    #[test]
    fn test_parse_frontmatter_valid() {
        let content = "---\nname: test\ndescription: desc\n---\n# Body";
        let (fm, body) = parse_frontmatter(content).unwrap();
        assert_eq!(fm.get("name").unwrap().as_str().unwrap(), "test");
        assert_eq!(body, "# Body");
    }

    #[test]
    fn test_parse_frontmatter_none() {
        assert!(parse_frontmatter("# Just markdown").is_none());
    }

    #[test]
    fn test_copy_dir_recursive() {
        let temp = TempDir::new().unwrap();
        let src = temp.path().join("src");
        let dest = temp.path().join("dest");
        fs::create_dir(&src).unwrap();
        fs::write(src.join("file.txt"), "hello").unwrap();
        fs::create_dir(src.join("sub")).unwrap();
        fs::write(src.join("sub/nested.txt"), "world").unwrap();

        copy_dir_recursive(&src, &dest).unwrap();
        assert!(dest.join("file.txt").exists());
        assert!(dest.join("sub/nested.txt").exists());
    }

    #[test]
    fn test_copy_dir_recursive_skips_hidden() {
        let temp = TempDir::new().unwrap();
        let src = temp.path().join("src");
        let dest = temp.path().join("dest");
        fs::create_dir(&src).unwrap();
        fs::write(src.join(".hidden"), "secret").unwrap();
        fs::write(src.join("visible"), "hello").unwrap();

        copy_dir_recursive(&src, &dest).unwrap();
        assert!(!dest.join(".hidden").exists());
        assert!(dest.join("visible").exists());
    }

    #[test]
    fn test_extract_zip() {
        let temp = TempDir::new().unwrap();
        let zip_path = temp.path().join("test.zip");
        let file = fs::File::create(&zip_path).unwrap();
        let mut zip = zip::ZipWriter::new(file);
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored);
        zip.start_file("skill/SKILL.md", options).unwrap();
        zip.write_all(b"---\nname: test\n---").unwrap();
        zip.finish().unwrap();

        let dest = temp.path().join("extracted");
        fs::create_dir(&dest).unwrap();
        extract_zip(&zip_path, &dest).unwrap();
        assert!(dest.join("skill/SKILL.md").exists());
    }

    #[test]
    fn test_is_zip_file() {
        let temp = TempDir::new().unwrap();
        let zip_path = temp.path().join("test.zip");
        fs::write(&zip_path, b"PK\x03\x04").unwrap();
        assert!(is_zip_file(&zip_path));

        let txt_path = temp.path().join("test.txt");
        fs::write(&txt_path, b"not a zip").unwrap();
        assert!(!is_zip_file(&txt_path));
    }

    #[test]
    fn test_hash_directory() {
        let temp = TempDir::new().unwrap();
        let dir = temp.path().join("skill");
        fs::create_dir(&dir).unwrap();
        fs::write(dir.join("file.txt"), "content").unwrap();

        let hash = hash_directory(&dir).unwrap();
        assert_eq!(hash.len(), 64);
    }

    #[test]
    fn test_hash_directory_deterministic() {
        let temp = TempDir::new().unwrap();
        let dir = temp.path().join("skill");
        fs::create_dir(&dir).unwrap();
        fs::write(dir.join("file.txt"), "content").unwrap();

        let h1 = hash_directory(&dir).unwrap();
        let h2 = hash_directory(&dir).unwrap();
        assert_eq!(h1, h2);
    }
}
