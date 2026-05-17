use crate::core::error::AppError;
use dirs::home_dir;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs;
use std::path::{Component, Path, PathBuf};
use walkdir::WalkDir;

pub const SKILL_MARKER_CANONICAL: &str = "SKILL.md";
pub const SKILL_MARKER_LEGACY: &str = "skill.md";
pub const SKILL_DIR_MARKERS: &[&str] = &[SKILL_MARKER_CANONICAL, SKILL_MARKER_LEGACY];

const SKILL_SCAN_EXCLUDE_DEFAULTS: &[&str] = &["node_modules", "target", "dist", "build"];

// Windows reserved device names. Skills on disk must avoid these even on POSIX
// because the central library is also synced to Windows tool dirs.
const WINDOWS_RESERVED: &[&str] = &[
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
    "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

pub fn find_skill_marker(dir: &Path) -> Option<PathBuf> {
    SKILL_DIR_MARKERS
        .iter()
        .map(|m| dir.join(m))
        .find(|p| p.exists())
}

pub fn is_valid_skill_dir(dir: &Path) -> bool {
    find_skill_marker(dir).is_some()
}

/// Validate a Git/ZIP `subpath` argument. Rejects absolute paths, `..`
/// components, Windows path prefixes, and control characters. Returns the
/// trimmed value on success.
pub fn validate_relative_subpath(raw: &str) -> Result<String, AppError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(AppError::InvalidSkill("Subpath cannot be empty".into()));
    }
    if trimmed.chars().any(|c| c.is_control()) {
        return Err(AppError::InvalidSkill(
            "Subpath contains control characters".into(),
        ));
    }
    for component in Path::new(trimmed).components() {
        match component {
            Component::ParentDir => {
                return Err(AppError::InvalidSkill(format!(
                    "Subpath '{}' contains '..' (path traversal)",
                    trimmed
                )));
            }
            Component::RootDir | Component::Prefix(_) => {
                return Err(AppError::InvalidSkill(format!(
                    "Subpath '{}' must be relative",
                    trimmed
                )));
            }
            _ => {}
        }
    }
    Ok(trimmed.to_string())
}

/// Strip a candidate skill name to something safe to use as a directory name
/// on every supported platform. Returns `None` when nothing usable remains
/// (empty after trimming, only path traversal, or matches a Windows reserved
/// device name).
pub fn sanitize_skill_name(raw: &str) -> Option<String> {
    // Use only the final path component to prevent `../evil` style escapes
    // even when a raw frontmatter name slips through.
    let last_component = Path::new(raw)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(raw);

    let trimmed = last_component.trim().trim_end_matches('.').trim();
    if trimmed.is_empty() || trimmed == "." || trimmed == ".." {
        return None;
    }

    let sanitized: String = trimmed
        .chars()
        .map(|c| {
            if c.is_control() || matches!(c, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*') {
                '_'
            } else {
                c
            }
        })
        .collect();

    let final_value = sanitized.trim().trim_end_matches('.').to_string();
    if final_value.is_empty() {
        return None;
    }

    let stem = final_value.split('.').next().unwrap_or(&final_value);
    if WINDOWS_RESERVED
        .iter()
        .any(|reserved| stem.eq_ignore_ascii_case(reserved))
    {
        return None;
    }

    Some(final_value)
}

/// Resolve a skill source directory inside an already-materialized tree
/// (e.g. a freshly cloned Git repository or an extracted ZIP).
///
/// Resolution order:
/// 1. If `subpath` is provided (non-empty), use `repo_dir.join(subpath)`.
///    The subpath is first checked with [`validate_relative_subpath`].
///    Returns an error when the path does not exist or is not a directory.
/// 2. If the repository root itself contains a SKILL.md/skill.md, use the root.
/// 3. Otherwise scan recursively (default excludes apply) for skill directories.
///    - Exactly one match → use it.
///    - Zero matches → error (no skill found).
///    - More than one match → error listing relative candidates so callers can
///      ask the user to specify a subpath.
pub fn resolve_skill_dir(repo_dir: &Path, subpath: Option<&str>) -> Result<PathBuf, AppError> {
    if let Some(sub) = subpath.map(str::trim).filter(|s| !s.is_empty()) {
        let validated = validate_relative_subpath(sub)?;
        let target = repo_dir.join(&validated);
        if !target.exists() || !target.is_dir() {
            return Err(AppError::InvalidSkill(format!(
                "Subpath '{}' not found in repository",
                validated
            )));
        }
        return Ok(target);
    }

    if is_valid_skill_dir(repo_dir) {
        return Ok(repo_dir.to_path_buf());
    }

    let candidates = find_skill_dirs(repo_dir, &[], true);
    match candidates.as_slice() {
        [] => Err(AppError::InvalidSkill(format!(
            "No SKILL.md or skill.md found in repository at {}",
            repo_dir.display()
        ))),
        [only] => Ok(only.clone()),
        many => {
            let rels: Vec<String> = many
                .iter()
                .filter_map(|p| p.strip_prefix(repo_dir).ok())
                .map(|p| p.to_string_lossy().replace('\\', "/"))
                .collect();
            Err(AppError::InvalidSkill(format!(
                "Multiple skills found in repository; specify a subpath. Candidates: {}",
                rels.join(", ")
            )))
        }
    }
}

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

    let frontmatter: HashMap<String, serde_json::Value> =
        serde_yaml::from_str(yaml_str).ok().unwrap_or_default();

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

    let mut walker = WalkDir::new(root)
        .max_depth(max_depth)
        .into_iter()
        .filter_entry(|entry| {
            let name = entry.file_name().to_string_lossy();
            if name.starts_with('.') {
                return false;
            }
            if SKILL_SCAN_EXCLUDE_DEFAULTS.iter().any(|d| name == *d) {
                return false;
            }
            if exclude.iter().any(|e| name == *e) {
                return false;
            }
            true
        });

    while let Some(entry) = walker.next() {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };
        if !entry.file_type().is_dir() {
            continue;
        }
        if is_valid_skill_dir(entry.path()) {
            results.push(entry.path().to_path_buf());
            // Do not recurse into a directory we just identified as a skill —
            // skills are not expected to nest inside other skills.
            walker.skip_current_dir();
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

    #[test]
    fn test_find_skill_marker_canonical() {
        let temp = TempDir::new().unwrap();
        let dir = temp.path().join("skill");
        fs::create_dir(&dir).unwrap();
        fs::write(dir.join("SKILL.md"), "---\nname: test\n---").unwrap();

        let marker = find_skill_marker(&dir);
        assert!(marker.is_some());
        assert_eq!(marker.unwrap().file_name().unwrap(), "SKILL.md");
    }

    #[test]
    fn test_find_skill_marker_legacy() {
        let temp = TempDir::new().unwrap();
        let dir = temp.path().join("skill");
        fs::create_dir(&dir).unwrap();
        fs::write(dir.join("skill.md"), "---\nname: test\n---").unwrap();

        let marker = find_skill_marker(&dir).expect("marker should be found");
        // On case-insensitive filesystems (default APFS/NTFS) the same file may
        // be reported as either "skill.md" or "SKILL.md"; both are acceptable.
        let name = marker.file_name().unwrap().to_string_lossy().to_lowercase();
        assert_eq!(name, "skill.md");
    }

    #[test]
    fn test_find_skill_marker_canonical_preferred() {
        let temp = TempDir::new().unwrap();
        let dir = temp.path().join("skill");
        fs::create_dir(&dir).unwrap();
        fs::write(dir.join("SKILL.md"), "canonical").unwrap();
        fs::write(dir.join("skill.md"), "legacy").unwrap();

        let marker = find_skill_marker(&dir);
        assert!(marker.is_some());
        assert_eq!(marker.unwrap().file_name().unwrap(), "SKILL.md");
    }

    #[test]
    fn test_find_skill_marker_none() {
        let temp = TempDir::new().unwrap();
        let dir = temp.path().join("not-skill");
        fs::create_dir(&dir).unwrap();
        fs::write(dir.join("README.md"), "# readme").unwrap();
        fs::write(dir.join("CLAUDE.md"), "# claude").unwrap();

        assert!(find_skill_marker(&dir).is_none());
    }

    #[test]
    fn test_is_valid_skill_dir() {
        let temp = TempDir::new().unwrap();

        let canonical = temp.path().join("a");
        fs::create_dir(&canonical).unwrap();
        fs::write(canonical.join("SKILL.md"), "").unwrap();
        assert!(is_valid_skill_dir(&canonical));

        let legacy = temp.path().join("b");
        fs::create_dir(&legacy).unwrap();
        fs::write(legacy.join("skill.md"), "").unwrap();
        assert!(is_valid_skill_dir(&legacy));

        let neither = temp.path().join("c");
        fs::create_dir(&neither).unwrap();
        fs::write(neither.join("README.md"), "").unwrap();
        assert!(!is_valid_skill_dir(&neither));
    }

    #[test]
    fn test_find_skill_dirs_detects_legacy() {
        let temp = TempDir::new().unwrap();
        let root = temp.path().join("root");
        fs::create_dir_all(root.join("legacy-skill")).unwrap();
        fs::write(root.join("legacy-skill/skill.md"), "---\nname: legacy\n---").unwrap();
        fs::create_dir_all(root.join("canonical-skill")).unwrap();
        fs::write(
            root.join("canonical-skill/SKILL.md"),
            "---\nname: canonical\n---",
        )
        .unwrap();

        let dirs = find_skill_dirs(&root, &[], false);
        assert_eq!(dirs.len(), 2);
    }

    #[test]
    fn test_find_skill_dirs_ignores_readme_and_claude() {
        let temp = TempDir::new().unwrap();
        let root = temp.path().join("root");
        // Namespace folder with only README/CLAUDE — must NOT be classified as skill.
        fs::create_dir_all(root.join("namespace")).unwrap();
        fs::write(root.join("namespace/README.md"), "# group").unwrap();
        fs::write(root.join("namespace/CLAUDE.md"), "# claude").unwrap();
        // Real nested skill underneath the namespace.
        fs::create_dir_all(root.join("namespace/real-skill")).unwrap();
        fs::write(
            root.join("namespace/real-skill/SKILL.md"),
            "---\nname: real\n---",
        )
        .unwrap();

        let dirs = find_skill_dirs(&root, &[], true);
        assert_eq!(dirs.len(), 1);
        assert!(dirs[0].ends_with("real-skill"));
    }

    #[test]
    fn test_find_skill_dirs_skips_excluded_defaults() {
        let temp = TempDir::new().unwrap();
        let root = temp.path().join("root");
        fs::create_dir_all(root.join("node_modules/pkg")).unwrap();
        fs::write(
            root.join("node_modules/pkg/SKILL.md"),
            "---\nname: junk\n---",
        )
        .unwrap();
        fs::create_dir_all(root.join("real-skill")).unwrap();
        fs::write(root.join("real-skill/SKILL.md"), "---\nname: real\n---").unwrap();

        let dirs = find_skill_dirs(&root, &[], true);
        assert_eq!(dirs.len(), 1);
        assert!(dirs[0].ends_with("real-skill"));
    }

    #[test]
    fn test_find_skill_dirs_does_not_recurse_into_skill() {
        let temp = TempDir::new().unwrap();
        let root = temp.path().join("root");
        fs::create_dir_all(root.join("outer/inner")).unwrap();
        fs::write(root.join("outer/SKILL.md"), "---\nname: outer\n---").unwrap();
        fs::write(root.join("outer/inner/SKILL.md"), "---\nname: inner\n---").unwrap();

        let dirs = find_skill_dirs(&root, &[], true);
        assert_eq!(dirs.len(), 1, "should not descend into a recognized skill");
        assert!(dirs[0].ends_with("outer"));
    }

    #[test]
    fn test_resolve_skill_dir_root_marker() {
        let temp = TempDir::new().unwrap();
        let repo = temp.path().join("repo");
        fs::create_dir(&repo).unwrap();
        fs::write(repo.join("SKILL.md"), "---\nname: x\n---").unwrap();

        let resolved = resolve_skill_dir(&repo, None).unwrap();
        assert_eq!(resolved, repo);
    }

    #[test]
    fn test_resolve_skill_dir_legacy_root_marker() {
        let temp = TempDir::new().unwrap();
        let repo = temp.path().join("repo");
        fs::create_dir(&repo).unwrap();
        fs::write(repo.join("skill.md"), "---\nname: x\n---").unwrap();

        let resolved = resolve_skill_dir(&repo, None).unwrap();
        assert_eq!(resolved, repo);
    }

    #[test]
    fn test_resolve_skill_dir_subpath() {
        let temp = TempDir::new().unwrap();
        let repo = temp.path().join("repo");
        fs::create_dir_all(repo.join("tools/web-search")).unwrap();
        fs::write(
            repo.join("tools/web-search/SKILL.md"),
            "---\nname: web\n---",
        )
        .unwrap();

        let resolved = resolve_skill_dir(&repo, Some("tools/web-search")).unwrap();
        assert_eq!(resolved, repo.join("tools/web-search"));
    }

    #[test]
    fn test_resolve_skill_dir_subpath_missing_errors() {
        let temp = TempDir::new().unwrap();
        let repo = temp.path().join("repo");
        fs::create_dir(&repo).unwrap();

        let err = resolve_skill_dir(&repo, Some("does/not/exist")).unwrap_err();
        assert!(matches!(err, AppError::InvalidSkill(_)));
    }

    #[test]
    fn test_resolve_skill_dir_single_nested_skill_auto_detect() {
        let temp = TempDir::new().unwrap();
        let repo = temp.path().join("repo");
        fs::create_dir_all(repo.join("skills/only-one")).unwrap();
        fs::write(repo.join("README.md"), "# repo").unwrap();
        fs::write(
            repo.join("skills/only-one/SKILL.md"),
            "---\nname: only\n---",
        )
        .unwrap();

        let resolved = resolve_skill_dir(&repo, None).unwrap();
        assert_eq!(resolved, repo.join("skills/only-one"));
    }

    #[test]
    fn test_resolve_skill_dir_multiple_skills_error() {
        let temp = TempDir::new().unwrap();
        let repo = temp.path().join("repo");
        fs::create_dir_all(repo.join("skills/a")).unwrap();
        fs::write(repo.join("skills/a/SKILL.md"), "---\nname: a\n---").unwrap();
        fs::create_dir_all(repo.join("skills/b")).unwrap();
        fs::write(repo.join("skills/b/SKILL.md"), "---\nname: b\n---").unwrap();

        let err = resolve_skill_dir(&repo, None).unwrap_err();
        let msg = match err {
            AppError::InvalidSkill(m) => m,
            other => panic!("expected InvalidSkill, got {:?}", other),
        };
        assert!(msg.contains("skills/a"));
        assert!(msg.contains("skills/b"));
    }

    #[test]
    fn test_resolve_skill_dir_none_found() {
        let temp = TempDir::new().unwrap();
        let repo = temp.path().join("repo");
        fs::create_dir(&repo).unwrap();
        fs::write(repo.join("README.md"), "# nothing here").unwrap();

        let err = resolve_skill_dir(&repo, None).unwrap_err();
        assert!(matches!(err, AppError::InvalidSkill(_)));
    }

    #[test]
    fn test_validate_relative_subpath_ok() {
        assert_eq!(
            validate_relative_subpath("tools/web-search").unwrap(),
            "tools/web-search"
        );
        assert_eq!(validate_relative_subpath("  skill ").unwrap(), "skill");
    }

    #[test]
    fn test_validate_relative_subpath_rejects_traversal() {
        let err = validate_relative_subpath("../etc/passwd").unwrap_err();
        assert!(matches!(err, AppError::InvalidSkill(_)));
        let err = validate_relative_subpath("a/../b").unwrap_err();
        assert!(matches!(err, AppError::InvalidSkill(_)));
    }

    #[test]
    fn test_validate_relative_subpath_rejects_absolute() {
        let err = validate_relative_subpath("/etc/passwd").unwrap_err();
        assert!(matches!(err, AppError::InvalidSkill(_)));
    }

    #[test]
    fn test_validate_relative_subpath_rejects_empty() {
        let err = validate_relative_subpath("   ").unwrap_err();
        assert!(matches!(err, AppError::InvalidSkill(_)));
    }

    #[test]
    fn test_resolve_skill_dir_rejects_traversal_subpath() {
        let temp = TempDir::new().unwrap();
        let repo = temp.path().join("repo");
        fs::create_dir_all(repo.join("legit")).unwrap();
        fs::write(repo.join("legit/SKILL.md"), "---\nname: x\n---").unwrap();

        let err = resolve_skill_dir(&repo, Some("../legit")).unwrap_err();
        assert!(matches!(err, AppError::InvalidSkill(_)));
    }

    #[test]
    fn test_sanitize_skill_name_basic() {
        assert_eq!(
            sanitize_skill_name("web-search").as_deref(),
            Some("web-search")
        );
        assert_eq!(
            sanitize_skill_name(" my skill ").as_deref(),
            Some("my skill")
        );
    }

    #[test]
    fn test_sanitize_skill_name_rejects_traversal() {
        assert_eq!(sanitize_skill_name(".."), None);
        assert_eq!(sanitize_skill_name("."), None);
        // Path component extraction strips parent dirs.
        assert_eq!(
            sanitize_skill_name("../../etc/passwd").as_deref(),
            Some("passwd")
        );
    }

    #[test]
    fn test_sanitize_skill_name_replaces_illegal_chars() {
        let cleaned = sanitize_skill_name("ev<il>na:me\"").unwrap();
        assert!(!cleaned.contains('<'));
        assert!(!cleaned.contains('>'));
        assert!(!cleaned.contains(':'));
        assert!(!cleaned.contains('"'));
    }

    #[test]
    fn test_sanitize_skill_name_rejects_control_chars_inline() {
        let cleaned = sanitize_skill_name("a\u{0007}b").unwrap();
        assert_eq!(cleaned, "a_b");
    }

    #[test]
    fn test_sanitize_skill_name_rejects_windows_reserved() {
        assert_eq!(sanitize_skill_name("CON"), None);
        assert_eq!(sanitize_skill_name("con"), None);
        assert_eq!(sanitize_skill_name("PRN.txt"), None);
        assert_eq!(sanitize_skill_name("LPT1"), None);
    }

    #[test]
    fn test_sanitize_skill_name_trailing_dots() {
        assert_eq!(sanitize_skill_name("name....").as_deref(), Some("name"));
        assert_eq!(sanitize_skill_name("...").as_deref(), None);
    }
}
