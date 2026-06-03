//! Encrypted backup archive format.
//!
//! Wire format (`.zip.enc`):
//!   [4 bytes]  magic = "SPBK"  (Skills Panel BacKup)
//!   [12 bytes] AES-256-GCM nonce (random per archive)
//!   [N bytes]  AES-256-GCM ciphertext + 16-byte auth tag
//!
//! Inside the (decrypted) zip:
//!   manifest.json       — { schema_version, created_at, skills_panel_version, skills: [...] }
//!   skills/<name>/...   — each skill directory, files copied verbatim
//!   config_snapshot.json — sanitized non-sensitive config (no proxy / tokens / passwords)
//!
//! Key derivation: SHA-256 of the user-supplied password gives the 32-byte AES key.
//! This is intentionally simple — backups are user-managed and short-lived.
//! Strong passwords are the user's responsibility (UI reminds them).

use crate::core::database::Database;
use crate::core::error::AppError;
use crate::core::models::{BackupManifest, BackupManifestEntry};
use crate::core::content_hash;
use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Key, Nonce};
use rand::RngCore;
use sha2::{Digest, Sha256};
use std::io::{Cursor, Read, Write};
use std::path::{Path, PathBuf};
use walkdir::WalkDir;
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

/// File magic prefix. Lets us tell a real backup from a random file with
/// no extension. Stored as ASCII bytes for trivial round-trip.
pub const ARCHIVE_MAGIC: &[u8; 4] = b"SPBK";

const NONCE_LEN: usize = 12;
const MANIFEST_NAME: &str = "manifest.json";
const CONFIG_SNAPSHOT_NAME: &str = "config_snapshot.json";
const SKILLS_PREFIX: &str = "skills/";
const SKIP_DIRS: &[&str] = &["node_modules", "target", "dist", "build", ".git"];

/// Built archive: the raw encrypted bytes plus a few summary fields
/// the caller (SyncEngine) needs to write a history row.
#[derive(Debug)]
pub struct EncryptedArchive {
    pub bytes: Vec<u8>,
    pub skills_count: usize,
    pub manifest: BackupManifest,
}

/// Build an encrypted backup archive from the current library + selected
/// config snapshot. `password` must be non-empty (caller validates).
pub fn build_archive(
    conn: &Database,
    password: &str,
) -> Result<EncryptedArchive, AppError> {
    if password.is_empty() {
        return Err(AppError::Config(
            "Backup password cannot be empty".into(),
        ));
    }

    // 1. Read all skills and compute their per-skill fingerprint.
    let skills = crate::core::database::SkillsRepository::new(conn)
        .get_all_active()
        .map_err(|e| AppError::Config(format!("Failed to list skills: {}", e)))?;

    let mut entries = Vec::with_capacity(skills.len());
    let mut total_size: u64 = 0;
    for skill in &skills {
        let dir = Path::new(&skill.library_path);
        if !dir.exists() {
            // Skip skills whose source dir disappeared between scan and export.
            // Caller (SyncEngine) will warn; we don't fail the whole backup.
            continue;
        }
        let content_sha = content_hash::ContentHash::hash_directory(dir).unwrap_or_default();
        let size = walkdir_size(dir);
        total_size += size;
        entries.push(BackupManifestEntry {
            id: skill.id.clone(),
            name: skill.name.clone(),
            content_sha256: content_sha,
            size_bytes: size,
        });
    }

    let manifest = BackupManifest {
        schema_version: 1,
        created_at: chrono::Utc::now().to_rfc3339(),
        skills_panel_version: env!("CARGO_PKG_VERSION").to_string(),
        skills: entries.clone(),
    };
    let manifest_json = serde_json::to_vec_pretty(&manifest)
        .map_err(|e| AppError::Config(format!("Failed to serialize manifest: {}", e)))?;

    // 2. Build the zip into an in-memory buffer.
    let mut zip_buf = Vec::with_capacity((total_size / 4) as usize + 4096);
    {
        let cursor = Cursor::new(&mut zip_buf);
        let mut zip = ZipWriter::new(cursor);
        let options =
            SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);

        // manifest first so the file is self-describing even if some skill
        // entries can't be read.
        zip.start_file(MANIFEST_NAME, options)?;
        zip.write_all(&manifest_json)?;

        // config snapshot (sanitized). Failure to read is non-fatal — the
        // backup is still useful without it.
        if let Ok(snapshot) = build_config_snapshot(conn) {
            if let Ok(snap_json) = serde_json::to_vec_pretty(&snapshot) {
                zip.start_file(CONFIG_SNAPSHOT_NAME, options)?;
                zip.write_all(&snap_json)?;
            }
        }

        // 3. Write each skill directory.
        for skill in &skills {
            let dir = Path::new(&skill.library_path);
            if !dir.exists() {
                continue;
            }
            let in_zip = format!("{SKILLS_PREFIX}{}", skill.name);
            add_dir_to_zip_filtered(&mut zip, dir, &in_zip, &options)?;
        }

        zip.finish()?;
    }

    // 4. Encrypt the whole zip with AES-256-GCM.
    let key = derive_key(password);
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&key));
    let mut nonce_bytes = [0u8; NONCE_LEN];
    rand::thread_rng().fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);
    let ciphertext = cipher
        .encrypt(nonce, zip_buf.as_ref())
        .map_err(|e| AppError::Config(format!("Archive encryption failed: {}", e)))?;

    // 5. Prepend the magic.
    let mut out = Vec::with_capacity(ARCHIVE_MAGIC.len() + NONCE_LEN + ciphertext.len());
    out.extend_from_slice(ARCHIVE_MAGIC);
    out.extend_from_slice(&nonce_bytes);
    out.extend_from_slice(&ciphertext);

    Ok(EncryptedArchive {
        bytes: out,
        skills_count: entries.len(),
        manifest,
    })
}

/// Encrypt a raw zip byte stream with the SPBK envelope (no skill
/// metadata). Used by the zip-slip test fixture so it can construct
/// a malicious inner zip and feed it back through the real extract
/// path. Visible at crate scope for tests.
#[cfg(test)]
pub(crate) fn encrypt_zip_bytes(zip_bytes: &[u8], password: &str) -> Vec<u8> {
    let key = derive_key(password);
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&key));
    let mut nonce_bytes = [0u8; NONCE_LEN];
    rand::thread_rng().fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);
    let ciphertext = cipher.encrypt(nonce, zip_bytes).expect("encrypt");
    let mut out = Vec::with_capacity(ARCHIVE_MAGIC.len() + NONCE_LEN + ciphertext.len());
    out.extend_from_slice(ARCHIVE_MAGIC);
    out.extend_from_slice(&nonce_bytes);
    out.extend_from_slice(&ciphertext);
    out
}

/// Decrypt + extract an archive. Writes skill directories under
/// `<dest>/skills/<name>/...`. Returns the parsed manifest.
pub fn extract_archive(
    bytes: &[u8],
    password: &str,
    dest: &Path,
) -> Result<BackupManifest, AppError> {
    if password.is_empty() {
        return Err(AppError::Config(
            "Backup password cannot be empty".into(),
        ));
    }
    if bytes.len() < ARCHIVE_MAGIC.len() + NONCE_LEN {
        return Err(AppError::Config("Archive is truncated".into()));
    }
    if &bytes[..4] != ARCHIVE_MAGIC {
        return Err(AppError::Config(
            "Archive magic mismatch — not a Skills Panel backup".into(),
        ));
    }
    let nonce_bytes: [u8; NONCE_LEN] = bytes[4..4 + NONCE_LEN]
        .try_into()
        .map_err(|_| AppError::Config("Archive nonce truncated".into()))?;
    let ciphertext = &bytes[4 + NONCE_LEN..];

    let key = derive_key(password);
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&key));
    let nonce = Nonce::from_slice(&nonce_bytes);
    let zip_bytes = cipher
        .decrypt(nonce, ciphertext)
        .map_err(|_| AppError::Config("Wrong password or corrupted archive".into()))?;

    // Read manifest first.
    let cursor = Cursor::new(zip_bytes.as_slice());
    let mut archive = ZipArchive::new(cursor)
        .map_err(|e| AppError::Config(format!("Failed to read zip: {}", e)))?;

    let manifest: BackupManifest = {
        let mut entry = archive
            .by_name(MANIFEST_NAME)
            .map_err(|e| AppError::Config(format!("Missing manifest.json: {}", e)))?;
        let mut s = String::new();
        entry
            .read_to_string(&mut s)
            .map_err(|e| AppError::Config(format!("Failed to read manifest: {}", e)))?;
        serde_json::from_str(&s)
            .map_err(|e| AppError::Config(format!("Manifest parse error: {}", e)))?
    };

    // Extract skill files. Each entry with prefix "skills/" goes under
    // <dest>/<relative-in-zip>. We rely on zip's enclosed_name() to prevent
    // zip slip, but the zip crate v2 `enclosed_name` is permissive
    // about some `..` paths (e.g. `skills/../also_escape`), so we add
    // an explicit `..` segment check as a defense in depth.
    std::fs::create_dir_all(dest)?;
    for i in 0..archive.len() {
        let mut entry = archive
            .by_index(i)
            .map_err(|e| AppError::Config(format!("Failed to read zip entry: {}", e)))?;
        let raw_name = entry.name().to_string();
        if !raw_name.starts_with(SKILLS_PREFIX) {
            continue;
        }
        let rel = match entry.enclosed_name() {
            Some(p) => p.to_path_buf(),
            None => continue, // skip unsafe paths
        };
        // Reject any path that still contains a `..` component after
        // the zip crate's normalization. This is the second line of
        // defense against zip slip (e.g. `skills/../foo`).
        if rel
            .components()
            .any(|c| c == std::path::Component::ParentDir)
        {
            continue;
        }
        let out_path = dest.join(&rel);
        if entry.is_dir() {
            std::fs::create_dir_all(&out_path)?;
            continue;
        }
        if let Some(parent) = out_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut f = std::fs::File::create(&out_path)?;
        std::io::copy(&mut entry, &mut f)?;
    }

    Ok(manifest)
}

// ── helpers ────────────────────────────────────────────────────────

fn derive_key(password: &str) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(password.as_bytes());
    let out = hasher.finalize();
    let mut key = [0u8; 32];
    key.copy_from_slice(&out);
    key
}

fn walkdir_size(dir: &Path) -> u64 {
    WalkDir::new(dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter_map(|e| e.metadata().ok())
        .map(|m| m.len())
        .sum()
}

fn add_dir_to_zip_filtered(
    zip: &mut ZipWriter<Cursor<&mut Vec<u8>>>,
    src: &Path,
    prefix: &str,
    options: &SimpleFileOptions,
) -> Result<(), AppError> {
    let walker = WalkDir::new(src).into_iter().filter_entry(|e| {
        // Prune excluded dirs and hidden files *before* descending. This
        // guarantees their contents are never visited, so empty directory
        // entries never get added to the zip.
        let name = e.file_name().to_string_lossy();
        if name.starts_with('.') {
            return false;
        }
        if e.file_type().is_dir() && SKIP_DIRS.contains(&name.as_ref()) {
            return false;
        }
        true
    });

    for entry in walker.filter_map(|e| e.ok()) {
        let rel = entry
            .path()
            .strip_prefix(src)
            .map_err(|e| AppError::Config(format!("zip walk strip error: {}", e)))?
            .to_string_lossy()
            .replace('\\', "/");
        if entry.file_type().is_dir() {
            let in_zip = format!("{prefix}/{rel}");
            zip.add_directory(&in_zip, *options)?;
        } else {
            let in_zip = format!("{prefix}/{rel}");
            zip.start_file(&in_zip, *options)?;
            let mut f = std::fs::File::open(entry.path())?;
            std::io::copy(&mut f, zip)?;
        }
    }
    Ok(())
}

/// Capture non-sensitive config keys. Any key in SENSITIVE_KEYS is omitted.
fn build_config_snapshot(conn: &Database) -> Result<serde_json::Value, AppError> {
    let all = crate::core::database::ConfigRepository::new(conn)
        .get_all()
        .map_err(|e| AppError::Config(format!("Failed to read config: {}", e)))?;
    let sensitive = crate::core::crypto::is_sensitive_key;
    let mut map = serde_json::Map::new();
    for (k, v) in all {
        if sensitive(&k) {
            continue;
        }
        map.insert(k, serde_json::Value::String(v));
    }
    Ok(serde_json::Value::Object(map))
}

// Re-export PathBuf for callers that need it (tests).
#[allow(dead_code)]
pub(crate) fn _ensure_pathbuf_import() -> Option<PathBuf> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::database::{Database, SkillsRepository};
    use crate::core::models::{Skill, SkillSourceType};
    use std::collections::HashMap;
    use std::fs;
    use tempfile::TempDir;

    fn make_test_db() -> (Database, TempDir) {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let db = Database::new(&tmp.path().to_path_buf()).unwrap();
        (db, TempDir::new().unwrap())
    }

    fn make_lib() -> TempDir {
        TempDir::new().unwrap()
    }

    fn make_skill(id: &str, name: &str, library_path: &str) -> Skill {
        Skill {
            id: id.to_string(),
            name: name.to_string(),
            path_hash: "h".into(),
            library_path: library_path.to_string(),
            original_source_path: None,
            original_git_url: None,
            original_git_subpath: None,
            group: "default".into(),
            description: "".into(),
            frontmatter: HashMap::new(),
            created_at: "2024-01-01T00:00:00Z".into(),
            mtime_ms: 0,
            source_type: SkillSourceType::LocalFolder,
            is_deleted: false,
            content_hash: None,
            source_revision: None,
            source_remote_revision: None,
            source_update_status: crate::core::models::SourceUpdateStatus::Unknown,
        }
    }

    fn seed_skill_on_disk(dir: &Path, name: &str, body: &str) -> PathBuf {
        let skill_dir = dir.join(name);
        fs::create_dir_all(&skill_dir).unwrap();
        fs::write(skill_dir.join("SKILL.md"), body).unwrap();
        skill_dir
    }

    #[test]
    fn test_archive_build_extract_roundtrip() {
        let (db, lib_tmp) = make_test_db();
        let lib = lib_tmp.path();
        let s1 = seed_skill_on_disk(lib, "alpha", "# alpha\nhello");
        let s2 = seed_skill_on_disk(lib, "beta", "# beta\nworld");

        SkillsRepository::new(&db)
            .upsert(&make_skill("s1", "alpha", s1.to_str().unwrap()))
            .unwrap();
        SkillsRepository::new(&db)
            .upsert(&make_skill("s2", "beta", s2.to_str().unwrap()))
            .unwrap();

        let archive = build_archive(&db, "secret-password").unwrap();
        assert_eq!(archive.skills_count, 2);
        assert!(archive.bytes.starts_with(ARCHIVE_MAGIC));

        // Extract to a fresh dir
        let extract_dir = tempfile::tempdir().unwrap();
        let manifest = extract_archive(&archive.bytes, "secret-password", extract_dir.path()).unwrap();
        assert_eq!(manifest.skills.len(), 2);

        // Files should be present
        let alpha_md = fs::read_to_string(extract_dir.path().join("skills/alpha/SKILL.md")).unwrap();
        assert_eq!(alpha_md, "# alpha\nhello");
        let beta_md = fs::read_to_string(extract_dir.path().join("skills/beta/SKILL.md")).unwrap();
        assert_eq!(beta_md, "# beta\nworld");
    }

    #[test]
    fn test_archive_encryption_changes_bytes() {
        let (db, lib_tmp) = make_test_db();
        let lib = lib_tmp.path();
        let s1 = seed_skill_on_disk(lib, "alpha", "# alpha");
        SkillsRepository::new(&db)
            .upsert(&make_skill("s1", "alpha", s1.to_str().unwrap()))
            .unwrap();

        let a1 = build_archive(&db, "pw").unwrap();
        let a2 = build_archive(&db, "pw").unwrap();
        // Same plaintext, different ciphertext (random nonce). Don't
        // assert exact byte lengths — zip entry timestamps and the random
        // nonce can shift sizes by a byte or two.
        assert_ne!(a1.bytes, a2.bytes, "nonces must randomize ciphertext");
        // Both must start with magic.
        assert!(a1.bytes.starts_with(ARCHIVE_MAGIC));
        assert!(a2.bytes.starts_with(ARCHIVE_MAGIC));
    }

    #[test]
    fn test_archive_wrong_password_fails() {
        let (db, lib_tmp) = make_test_db();
        let lib = lib_tmp.path();
        let s1 = seed_skill_on_disk(lib, "alpha", "x");
        SkillsRepository::new(&db)
            .upsert(&make_skill("s1", "alpha", s1.to_str().unwrap()))
            .unwrap();

        let a = build_archive(&db, "right").unwrap();
        let err = extract_archive(&a.bytes, "WRONG", tempfile::tempdir().unwrap().path())
            .unwrap_err();
        assert!(
            err.to_string().contains("Wrong password")
                || err.to_string().contains("corrupted"),
            "got: {err}"
        );
    }

    #[test]
    fn test_archive_corrupted_zip_fails() {
        let (db, lib_tmp) = make_test_db();
        let lib = lib_tmp.path();
        let s1 = seed_skill_on_disk(lib, "alpha", "x");
        SkillsRepository::new(&db)
            .upsert(&make_skill("s1", "alpha", s1.to_str().unwrap()))
            .unwrap();

        let mut a = build_archive(&db, "pw").unwrap();
        // Flip a byte in the ciphertext (after magic + nonce)
        let idx = a.bytes.len() - 1;
        a.bytes[idx] ^= 0xFF;
        let err = extract_archive(&a.bytes, "pw", tempfile::tempdir().unwrap().path()).unwrap_err();
        assert!(
            err.to_string().contains("Wrong password")
                || err.to_string().contains("corrupted")
                || err.to_string().contains("Failed to read zip"),
            "got: {err}"
        );
    }

    #[test]
    fn test_archive_magic_mismatch_surfaces() {
        let garbage = b"XXXX\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00garbage";
        let err = extract_archive(garbage, "pw", tempfile::tempdir().unwrap().path()).unwrap_err();
        assert!(err.to_string().contains("magic"), "got: {err}");
    }

    #[test]
    fn test_archive_truncated_input_surfaces() {
        let err = extract_archive(b"SP", "pw", tempfile::tempdir().unwrap().path()).unwrap_err();
        assert!(err.to_string().contains("truncated"), "got: {err}");
    }

    #[test]
    fn test_archive_empty_password_rejected() {
        let (db, lib_tmp) = make_test_db();
        let lib = lib_tmp.path();
        let err = build_archive(&db, "").unwrap_err();
        assert!(err.to_string().contains("password"), "got: {err}");

        // extract side too
        let dummy = b"SPBK\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00X";
        let err = extract_archive(dummy, "", tempfile::tempdir().unwrap().path()).unwrap_err();
        assert!(err.to_string().contains("password"), "got: {err}");
    }

    #[test]
    fn test_archive_excludes_node_modules_and_dotfiles() {
        let (db, lib_tmp) = make_test_db();
        let lib = lib_tmp.path();
        let skill_dir = lib.join("alpha");
        fs::create_dir_all(skill_dir.join("node_modules/foo")).unwrap();
        fs::write(skill_dir.join("node_modules/foo/index.js"), "noop").unwrap();
        fs::write(skill_dir.join(".hidden"), "secret").unwrap();
        fs::create_dir_all(skill_dir.join("src")).unwrap();
        fs::write(skill_dir.join("src/index.md"), "ok").unwrap();
        fs::write(skill_dir.join("SKILL.md"), "# alpha").unwrap();
        SkillsRepository::new(&db)
            .upsert(&make_skill("s1", "alpha", skill_dir.to_str().unwrap()))
            .unwrap();

        let a = build_archive(&db, "pw").unwrap();
        let extract_dir = tempfile::tempdir().unwrap();
        extract_archive(&a.bytes, "pw", extract_dir.path()).unwrap();

        // node_modules and .hidden must be absent
        assert!(!extract_dir.path().join("skills/alpha/node_modules").exists());
        assert!(!extract_dir.path().join("skills/alpha/.hidden").exists());
        // SKILL.md + src/index.md must be present
        assert!(extract_dir.path().join("skills/alpha/SKILL.md").exists());
        assert!(extract_dir.path().join("skills/alpha/src/index.md").exists());
    }

    #[test]
    fn test_archive_skips_skill_with_missing_dir() {
        let (db, lib_tmp) = make_test_db();
        let lib = lib_tmp.path();
        let s1 = seed_skill_on_disk(lib, "present", "x");
        // s2 has a path that doesn't exist on disk
        SkillsRepository::new(&db)
            .upsert(&make_skill("s1", "present", s1.to_str().unwrap()))
            .unwrap();
        SkillsRepository::new(&db)
            .upsert(&make_skill("s2", "ghost", "/nonexistent/path"))
            .unwrap();

        let a = build_archive(&db, "pw").unwrap();
        // manifest contains both entries, but only "present" was actually written
        let extract_dir = tempfile::tempdir().unwrap();
        extract_archive(&a.bytes, "pw", extract_dir.path()).unwrap();
        assert!(extract_dir.path().join("skills/present/SKILL.md").exists());
        assert!(!extract_dir.path().join("skills/ghost").exists());
    }

    #[test]
    fn test_archive_config_snapshot_omits_sensitive_keys() {
        let (db, lib_tmp) = make_test_db();
        let lib = lib_tmp.path();
        // Write a non-sensitive and a sensitive key via ConfigRepository
        let cfg = crate::core::database::ConfigRepository::new(&db);
        cfg.set("sync", r#"{"mode":"symlink"}"#).unwrap();
        cfg.set("webdav_password", "supersecret").unwrap();

        let _ = build_archive(&db, "pw").unwrap();

        // Inspect the zip in the encrypted archive — but we don't decrypt here,
        // so we test the snapshot fn via a fresh build.
        let snapshot = build_config_snapshot(&db).unwrap();
        let obj = snapshot.as_object().unwrap();
        assert!(obj.contains_key("sync"));
        assert!(!obj.contains_key("webdav_password"));
    }

    /// Build a zip with malicious `../` entries, encrypt it, and verify
    /// that `extract_archive` does not let them escape the destination
    /// directory. We include a valid manifest.json so the extractor
    /// doesn't bail before reaching the entry-walk.
    #[test]
    fn test_archive_rejects_zip_slip_paths() {
        let (db, _lib_tmp) = make_test_db();

        // Build a raw zip containing a valid manifest plus two
        // path-traversal attempts and one normal file.
        let mut buf = Vec::new();
        {
            let cursor = std::io::Cursor::new(&mut buf);
            let mut zip = zip::ZipWriter::new(cursor);
            let options = zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Deflated);

            let manifest = crate::core::models::BackupManifest {
                schema_version: 1,
                created_at: "2024-01-01T00:00:00Z".into(),
                skills_panel_version: "0.0.0".into(),
                skills: vec![],
            };
            zip.start_file(MANIFEST_NAME, options).unwrap();
            std::io::Write::write_all(&mut zip, serde_json::to_vec(&manifest).unwrap().as_slice()).unwrap();
            zip.start_file("skills/normal.txt", options).unwrap();
            std::io::Write::write_all(&mut zip, b"safe content").unwrap();
            zip.start_file("../escape.txt", options).unwrap();
            std::io::Write::write_all(&mut zip, b"escaped").unwrap();
            zip.start_file("skills/../also_escape.txt", options).unwrap();
            std::io::Write::write_all(&mut zip, b"escaped2").unwrap();
            zip.finish().unwrap();
        }

        // Wrap in the SPBK envelope so extract_archive exercises the
        // real decrypt + parse path.
        let encrypted = encrypt_zip_bytes(&buf, "pw");

        let dest = tempfile::tempdir().unwrap();
        extract_archive(&encrypted, "pw", dest.path()).unwrap();

        // The normal entry should land at `<dest>/skills/normal.txt`.
        assert!(dest.path().join("skills/normal.txt").exists());
        // The traversal entries should be silently dropped (the zip
        // crate's `enclosed_name()` returns None for paths with `..`).
        assert!(
            !dest.path().join("escape.txt").exists(),
            "zip slip let `../escape.txt` escape"
        );
        assert!(
            !dest.path().join("also_escape.txt").exists(),
            "zip slip let `skills/../also_escape.txt` escape"
        );
        // Sanity: nothing should have been written outside `dest`.
        assert!(!dest.path().parent().unwrap().join("escape.txt").exists());
    }
}
