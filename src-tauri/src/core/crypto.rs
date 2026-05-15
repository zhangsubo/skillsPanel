use crate::core::error::AppError;
use aes_gcm::{
    aead::{Aead, KeyInit, OsRng},
    Aes256Gcm, Nonce,
};
use rand::RngCore;
use std::fs;
use std::path::Path;

const ENC_PREFIX: &str = "ENC:";
const KEY_FILE_NAME: &str = ".secret.key";

pub struct Crypto {
    cipher: Aes256Gcm,
}

impl Crypto {
    pub fn new(key_dir: &Path) -> Result<Self, AppError> {
        let key_path = key_dir.join(KEY_FILE_NAME);
        let key = if key_path.exists() {
            let key_bytes = fs::read(&key_path)
                .map_err(|e| AppError::Config(format!("Failed to read secret key: {}", e)))?;
            if key_bytes.len() != 32 {
                return Err(AppError::Config("Invalid secret key length".into()));
            }
            let mut key = [0u8; 32];
            key.copy_from_slice(&key_bytes);
            key
        } else {
            let mut key = [0u8; 32];
            OsRng.fill_bytes(&mut key);
            fs::create_dir_all(key_dir)
                .map_err(|e| AppError::Config(format!("Failed to create key directory: {}", e)))?;
            fs::write(&key_path, &key)
                .map_err(|e| AppError::Config(format!("Failed to write secret key: {}", e)))?;
            key
        };

        let cipher = Aes256Gcm::new_from_slice(&key)
            .map_err(|e| AppError::Config(format!("Failed to create cipher: {}", e)))?;

        Ok(Self { cipher })
    }

    pub fn encrypt(&self, plaintext: &str) -> Result<String, AppError> {
        let mut nonce_bytes = [0u8; 12];
        OsRng.fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);

        let ciphertext = self
            .cipher
            .encrypt(nonce, plaintext.as_bytes())
            .map_err(|e| AppError::Config(format!("Encryption failed: {}", e)))?;

        let mut result = Vec::with_capacity(12 + ciphertext.len());
        result.extend_from_slice(&nonce_bytes);
        result.extend_from_slice(&ciphertext);

        Ok(format!("ENC:{}", base64_encode(&result)))
    }

    pub fn decrypt(&self, stored: &str) -> Result<String, AppError> {
        if !stored.starts_with(ENC_PREFIX) {
            return Ok(stored.to_string());
        }

        let encoded = &stored[ENC_PREFIX.len()..];
        let data = base64_decode(encoded)
            .map_err(|e| AppError::Config(format!("Invalid encrypted data: {}", e)))?;

        if data.len() < 12 {
            return Err(AppError::Config("Invalid encrypted data length".into()));
        }

        let (nonce_bytes, ciphertext) = data.split_at(12);
        let nonce = Nonce::from_slice(nonce_bytes);

        let plaintext = self
            .cipher
            .decrypt(nonce, ciphertext)
            .map_err(|e| AppError::Config(format!("Decryption failed: {}", e)))?;

        String::from_utf8(plaintext)
            .map_err(|e| AppError::Config(format!("Invalid UTF-8 in decrypted data: {}", e)))
    }

    pub fn is_encrypted(value: &str) -> bool {
        value.starts_with(ENC_PREFIX)
    }
}

fn base64_encode(data: &[u8]) -> String {
    const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut result = String::with_capacity((data.len() + 2) / 3 * 4);
    for chunk in data.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = if chunk.len() > 1 { chunk[1] as u32 } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] as u32 } else { 0 };
        let triple = (b0 << 16) | (b1 << 8) | b2;
        result.push(CHARS[((triple >> 18) & 0x3F) as usize] as char);
        result.push(CHARS[((triple >> 12) & 0x3F) as usize] as char);
        if chunk.len() > 1 {
            result.push(CHARS[((triple >> 6) & 0x3F) as usize] as char);
        } else {
            result.push('=');
        }
        if chunk.len() > 2 {
            result.push(CHARS[(triple & 0x3F) as usize] as char);
        } else {
            result.push('=');
        }
    }
    result
}

fn base64_decode(encoded: &str) -> Result<Vec<u8>, AppError> {
    let encoded = encoded.trim_end_matches('=');
    let mut result = Vec::with_capacity(encoded.len() * 3 / 4);
    let mut buf = 0u32;
    let mut bits = 0;

    for byte in encoded.bytes() {
        let val = match byte {
            b'A'..=b'Z' => byte - b'A',
            b'a'..=b'z' => byte - b'a' + 26,
            b'0'..=b'9' => byte - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            _ => return Err(AppError::Config("Invalid base64 character".into())),
        };
        buf = (buf << 6) | val as u32;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            result.push((buf >> bits) as u8);
        }
    }

    Ok(result)
}

const SENSITIVE_KEYS: &[&str] = &["proxy_url", "skillsmp_api_key", "git_backup_remote_url"];

pub fn is_sensitive_key(key: &str) -> bool {
    SENSITIVE_KEYS.contains(&key)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_encrypt_decrypt_roundtrip() {
        let temp = TempDir::new().unwrap();
        let crypto = Crypto::new(temp.path()).unwrap();

        let plaintext = "my-secret-api-key-12345";
        let encrypted = crypto.encrypt(plaintext).unwrap();
        assert!(encrypted.starts_with("ENC:"));
        assert_ne!(encrypted, plaintext);

        let decrypted = crypto.decrypt(&encrypted).unwrap();
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_decrypt_plaintext_passthrough() {
        let temp = TempDir::new().unwrap();
        let crypto = Crypto::new(temp.path()).unwrap();

        let plaintext = "not-encrypted-value";
        let result = crypto.decrypt(plaintext).unwrap();
        assert_eq!(result, plaintext);
    }

    #[test]
    fn test_is_encrypted() {
        assert!(Crypto::is_encrypted("ENC:abc123"));
        assert!(!Crypto::is_encrypted("plain text"));
        assert!(!Crypto::is_encrypted(""));
    }

    #[test]
    fn test_key_persistence() {
        let temp = TempDir::new().unwrap();
        let crypto1 = Crypto::new(temp.path()).unwrap();
        let encrypted = crypto1.encrypt("test").unwrap();

        let crypto2 = Crypto::new(temp.path()).unwrap();
        let decrypted = crypto2.decrypt(&encrypted).unwrap();
        assert_eq!(decrypted, "test");
    }

    #[test]
    fn test_different_plaintexts_produce_different_ciphertexts() {
        let temp = TempDir::new().unwrap();
        let crypto = Crypto::new(temp.path()).unwrap();

        let enc1 = crypto.encrypt("same").unwrap();
        let enc2 = crypto.encrypt("same").unwrap();
        assert_ne!(enc1, enc2);
    }
}
