//! WebDavProvider — backup target = any RFC 4918 WebDAV server.
//!
//! We hand-roll a tiny WebDAV client on top of `reqwest::blocking` rather
//! than depending on a community crate (which is in varying states of
//! maintenance). The subset we need:
//!
//!   MKCOL   ensure the remote dir exists
//!   PROPFIND list files in a dir
//!   PUT     upload a file
//!   GET     download a file
//!   DELETE  not currently used (kept here for symmetry / future cleanup)
//!
//! Auth: HTTP Basic. The username/password are stored separately in
//! `webdav_username` / `webdav_password` SENSITIVE_KEYs (encrypted via
//! ConfigRepository), NOT in the provider's `config_json`.

use crate::core::error::AppError;
use crate::core::sync::provider::{ProviderInfo, RemoteFile, SyncProvider};
use base64::Engine;
use reqwest::blocking::Client;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue, AUTHORIZATION, CONTENT_TYPE, USER_AGENT};
use reqwest::StatusCode;
use std::time::Duration;

pub struct WebDavProvider {
    pub base_url: String,   // e.g. "https://nextcloud.example.com/remote.php/dav/files/alice"
    pub username: String,
    pub password: String,
    /// Sub-directory under base_url where archives live. Empty = root.
    pub remote_path: String, // e.g. "backups"
}

impl WebDavProvider {
    pub fn new(
        base_url: impl Into<String>,
        username: impl Into<String>,
        password: impl Into<String>,
        remote_path: impl Into<String>,
    ) -> Self {
        Self {
            base_url: base_url.into(),
            username: username.into(),
            password: password.into(),
            remote_path: remote_path.into(),
        }
    }

    fn client(&self) -> Result<Client, AppError> {
        Client::builder()
            .timeout(Duration::from_secs(60))
            .user_agent(USER_AGENT_VALUE)
            .build()
            .map_err(|e| AppError::Config(format!("http client: {e}")))
    }

    fn auth_header(&self) -> Result<HeaderValue, AppError> {
        let raw = format!("{}:{}", self.username, self.password);
        let encoded = base64::engine::general_purpose::STANDARD.encode(raw);
        HeaderValue::from_str(&format!("Basic {encoded}"))
            .map_err(|e| AppError::Config(format!("auth header: {e}")))
    }

    /// Build a full URL for a sub-resource. `sub` is relative to the
    /// remote_path; pass "" to get the remote_path directory itself.
    fn full_url(&self, sub: &str) -> String {
        let base = self.base_url.trim_end_matches('/');
        let dir = self.remote_path.trim_matches('/');
        if sub.is_empty() {
            if dir.is_empty() {
                format!("{base}/")
            } else {
                format!("{base}/{dir}/")
            }
        } else {
            if dir.is_empty() {
                format!("{base}/{sub}")
            } else {
                format!("{base}/{dir}/{sub}")
            }
        }
    }

    fn ensure_dir(&self) -> Result<(), AppError> {
        let client = self.client()?;
        let mut headers = HeaderMap::new();
        headers.insert(AUTHORIZATION, self.auth_header()?);
        let dir_url = self.full_url("");
        let res = client
            .request(reqwest::Method::from_bytes(b"MKCOL").unwrap(), &dir_url)
            .headers(headers.clone())
            .send()
            .map_err(|e| AppError::Config(format!("MKCOL {dir_url}: {e}")))?;
        match res.status() {
            s if s.is_success() || s == StatusCode::CREATED || s == StatusCode::METHOD_NOT_ALLOWED => {
                // 405 means the collection already exists on some servers.
                Ok(())
            }
            s => {
                // Some servers (Apache mod_dav) return 301/207 etc. for
                // existing dirs. Treat 2xx, 3xx (redirect), and 405 as ok.
                if s.is_redirection() || s == StatusCode::METHOD_NOT_ALLOWED {
                    Ok(())
                } else {
                    let body = res.text().unwrap_or_default();
                    Err(AppError::Config(format!(
                        "MKCOL {dir_url} returned {s}: {}",
                        body.chars().take(200).collect::<String>()
                    )))
                }
            }
        }
    }
}

const USER_AGENT_VALUE: &str = "SkillsPanel/0.6.3";

impl SyncProvider for WebDavProvider {
    fn kind(&self) -> &'static str {
        "webdav"
    }

    fn prepare_remote(&self) -> Result<(), AppError> {
        self.ensure_dir()
    }

    fn upload(&self, bytes: &[u8], filename: &str) -> Result<(), AppError> {
        self.ensure_dir()?;
        let client = self.client()?;
        let url = self.full_url(filename);
        let mut headers = HeaderMap::new();
        headers.insert(AUTHORIZATION, self.auth_header()?);
        // Some servers (SabreDAV) require Content-Type to be set explicitly.
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/octet-stream"));
        let res = client
            .put(&url)
            .headers(headers)
            .body(bytes.to_vec())
            .send()
            .map_err(|e| AppError::Config(format!("PUT {url}: {e}")))?;
        if !res.status().is_success() {
            let status = res.status();
            let body = res.text().unwrap_or_default();
            return Err(AppError::Config(format!(
                "PUT {url} returned {status}: {}",
                body.chars().take(200).collect::<String>()
            )));
        }
        Ok(())
    }

    fn download_latest(&self) -> Result<(Vec<u8>, String), AppError> {
        let files = self.list_remote()?;
        let latest = files
            .last()
            .ok_or_else(|| AppError::Config("No backups found on WebDAV server".into()))?;
        let client = self.client()?;
        let url = self.full_url(&latest.name);
        let mut headers = HeaderMap::new();
        headers.insert(AUTHORIZATION, self.auth_header()?);
        let res = client
            .get(&url)
            .headers(headers)
            .send()
            .map_err(|e| AppError::Config(format!("GET {url}: {e}")))?;
        if !res.status().is_success() {
            let status = res.status();
            return Err(AppError::Config(format!("GET {url} returned {status}")));
        }
        let bytes = res
            .bytes()
            .map_err(|e| AppError::Config(format!("read body: {e}")))?
            .to_vec();
        Ok((bytes, latest.name.clone()))
    }

    fn list_remote(&self) -> Result<Vec<RemoteFile>, AppError> {
        let client = self.client()?;
        let url = self.full_url("");
        let mut headers = HeaderMap::new();
        headers.insert(AUTHORIZATION, self.auth_header()?);
        headers.insert(
            HeaderName::from_static("depth"),
            HeaderValue::from_static("1"),
        );
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/xml"));
        let body = r#"<?xml version="1.0" encoding="utf-8"?>
            <d:propfind xmlns:d="DAV:">
              <d:prop>
                <d:getcontentlength/>
                <d:getlastmodified/>
                <d:resourcetype/>
              </d:prop>
            </d:propfind>"#;
        let res = client
            .request(reqwest::Method::from_bytes(b"PROPFIND").unwrap(), &url)
            .headers(headers)
            .body(body)
            .send()
            .map_err(|e| AppError::Config(format!("PROPFIND {url}: {e}")))?;
        let status = res.status();
        let text = res
            .text()
            .map_err(|e| AppError::Config(format!("read propfind body: {e}")))?;
        if !status.is_success() {
            return Err(AppError::Config(format!(
                "PROPFIND {url} returned {status}: {}",
                text.chars().take(200).collect::<String>()
            )));
        }
        let out = parse_propfind_response(&text, &url);
        Ok(out)
    }

    fn test_connection(&self) -> Result<(), AppError> {
        // list_remote is the strongest connectivity check: it requires
        // auth to succeed, and the response to be well-formed XML.
        self.list_remote().map(|_| ())
    }
}

use crate::core::sync::provider::ConfigField;

/// Minimal PROPFIND multistatus XML parser. Extracts the file/dir names
/// under `<response>` that are NOT the collection itself. Pulls out
/// content length and last-modified when present.
fn parse_propfind_response(xml: &str, base_url: &str) -> Vec<RemoteFile> {
    let mut out = Vec::new();
    let base_normalized = base_url.trim_end_matches('/').to_string();
    for resp in split_response_blocks(xml) {
        // Find href by trying d:href and bare href.
        let href = extract_tag(&resp, "d:href")
            .or_else(|| extract_tag(&resp, "href"))
            .unwrap_or_default();
        let href_decoded = percent_decode(&href);
        let href_trimmed = href_decoded.trim_end_matches('/');
        if href_trimmed.is_empty() {
            continue;
        }
        if href_trimmed == base_normalized {
            continue;
        }
        // Skip directories: resourcetype contains a <...collection/> child.
        // Use prefix-agnostic contains for both forms.
        if resp.contains("collection")
            && (resp.contains("<d:collection") || resp.contains("<collection"))
        {
            continue;
        }
        let name = href_trimmed
            .rsplit('/')
            .next()
            .unwrap_or(href_trimmed)
            .to_string();
        if name.is_empty() {
            continue;
        }
        let size = extract_tag(&resp, "d:getcontentlength")
            .or_else(|| extract_tag(&resp, "getcontentlength"))
            .and_then(|s| s.trim().parse::<u64>().ok())
            .unwrap_or(0);
        let last_modified = extract_tag(&resp, "d:getlastmodified")
            .or_else(|| extract_tag(&resp, "getlastmodified"));
        out.push(RemoteFile {
            name,
            size,
            last_modified,
        });
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

fn split_response_blocks(xml: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = xml;
    while let Some(open_pos) = find_response_open(rest) {
        let after_open = &rest[open_pos.0..];
        let open_tag_end = after_open.find('>').map(|i| i + 1).unwrap_or(0);
        let content_start = open_pos.0 + open_tag_end;
        let close_pos = match find_response_close(&rest[content_start..]) {
            Some((c, _)) => c,
            None => break,
        };
        let block_end = content_start + close_pos;
        out.push(rest[content_start..block_end].to_string());
        rest = &rest[block_end..];
        if let Some(close_tag_end) = rest.find('>') {
            rest = &rest[close_tag_end + 1..];
        } else {
            break;
        }
    }
    out
}

/// Find the next opening `<...response>` tag. Returns (absolute_offset, prefix_len).
fn find_response_open(s: &str) -> Option<(usize, usize)> {
    // Look for `<` followed by an optional `[a-z]+:` prefix, then `response`.
    // The full match is `<[prefix:]response` plus the rest of the tag up to `>`.
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'<' {
            // Try to match a response tag at position i.
            let mut j = i + 1;
            // Skip optional prefix like "d:" or "D:".
            while j < bytes.len() && (bytes[j] as char).is_ascii_alphabetic() {
                j += 1;
            }
            if j < bytes.len() && bytes[j] == b':' {
                j += 1;
            }
            // Skip tag attributes (everything up to whitespace or `>`).
            let mut k = j;
            while k < bytes.len() && bytes[k] != b'>' && !bytes[k].is_ascii_whitespace() {
                k += 1;
            }
            let tag_name = &s[j..k];
            if tag_name == "response" {
                return Some((i, j - i));
            }
            // Not a match; continue from j.
            i = j.max(i + 1);
        } else {
            i += 1;
        }
    }
    None
}

/// Find the next closing `</...response>` tag. Returns (offset, total_tag_len).
fn find_response_close(s: &str) -> Option<(usize, usize)> {
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if i + 1 < bytes.len() && bytes[i] == b'<' && bytes[i + 1] == b'/' {
            let mut j = i + 2;
            while j < bytes.len() && (bytes[j] as char).is_ascii_alphabetic() {
                j += 1;
            }
            if j < bytes.len() && bytes[j] == b':' {
                j += 1;
            }
            let mut k = j;
            while k < bytes.len() && bytes[k] != b'>' && !bytes[k].is_ascii_whitespace() {
                k += 1;
            }
            let tag_name = &s[j..k];
            if tag_name == "response" {
                return Some((i, k - i));
            }
            i = k.max(i + 1);
        } else {
            i += 1;
        }
    }
    None
}

fn extract_tag(xml: &str, tag: &str) -> Option<String> {
    let open = format!("<{tag}>");
    let close = format!("</{tag}>");
    let start = xml.find(&open)? + open.len();
    let end = xml[start..].find(&close)? + start;
    Some(xml[start..end].to_string())
}

fn percent_decode(s: &str) -> String {
    // Minimal percent-decode. Real URL decode handles UTF-8 + edge cases;
    // this is enough for WebDAV hrefs that contain %20 etc.
    let mut out = String::with_capacity(s.len());
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let (Some(h), Some(l)) = (
                (bytes[i + 1] as char).to_digit(16),
                (bytes[i + 2] as char).to_digit(16),
            ) {
                out.push((h * 16 + l) as u8 as char);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i] as char);
        i += 1;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use mockito::Server;

    fn url(s: &str) -> String {
        s.trim_end_matches('/').to_string()
    }

    fn make_provider(server_url: &str) -> WebDavProvider {
        WebDavProvider::new(server_url, "alice", "secret", "backups")
    }

    #[test]
    fn test_webdav_provider_mkcol_then_idempotent() {
        let mut server = Server::new();
        let _m1 = server
            .mock("MKCOL", "/backups/")
            .with_status(201)
            .create();
        let p = make_provider(&server.url());
        p.prepare_remote().unwrap();
    }

    #[test]
    fn test_webdav_provider_put_uploads_encrypted_blob() {
        let mut server = Server::new();
        let _mk = server
            .mock("MKCOL", "/backups/")
            .with_status(201)
            .create();
        let _m = server
            .mock("PUT", "/backups/test.zip.enc")
            .match_body("HELLO")
            .with_status(201)
            .create();
        let p = make_provider(&server.url());
        p.upload(b"HELLO", "test.zip.enc").unwrap();
    }

    #[test]
    fn test_webdav_provider_get_downloads_latest() {
        let mut server = Server::new();
        let _m1 = server
            .mock("MKCOL", "/backups/")
            .with_status(201)
            .create();
        let _m2 = server
            .mock("PROPFIND", "/backups/")
            .with_status(207)
            .with_header("content-type", "application/xml")
            .with_body(
                r#"<?xml version="1.0" encoding="utf-8"?>
                <d:multistatus xmlns:d="DAV:">
                  <d:response>
                    <d:href>/backups/</d:href>
                    <d:propstat><d:prop><d:resourcetype><d:collection/></d:resourcetype></d:prop></d:propstat>
                  </d:response>
                  <d:response>
                    <d:href>/backups/20260101-000001.zip.enc</d:href>
                    <d:propstat>
                      <d:prop>
                        <d:getcontentlength>5</d:getcontentlength>
                        <d:getlastmodified>Mon, 01 Jan 2026 00:00:01 GMT</d:getlastmodified>
                        <d:resourcetype/>
                      </d:prop>
                    </d:propstat>
                  </d:response>
                  <d:response>
                    <d:href>/backups/20260102-000002.zip.enc</d:href>
                    <d:propstat>
                      <d:prop>
                        <d:getcontentlength>5</d:getcontentlength>
                        <d:getlastmodified>Wed, 02 Jan 2026 00:00:02 GMT</d:getlastmodified>
                        <d:resourcetype/>
                      </d:prop>
                    </d:propstat>
                  </d:response>
                </d:multistatus>"#,
            )
            .create();
        let _m3 = server
            .mock("GET", "/backups/20260102-000002.zip.enc")
            .with_status(200)
            .with_body("WORLD")
            .create();
        let p = make_provider(&server.url());
        let (bytes, name) = p.download_latest().unwrap();
        assert_eq!(name, "20260102-000002.zip.enc");
        assert_eq!(bytes, b"WORLD");
    }

    #[test]
    fn test_webdav_provider_list_returns_remote_files_only() {
        let mut server = Server::new();
        let _m = server
            .mock("PROPFIND", "/backups/")
            .with_status(207)
            .with_header("content-type", "application/xml")
            .with_body(
                r#"<?xml version="1.0" encoding="utf-8"?>
                <d:multistatus xmlns:d="DAV:">
                  <d:response>
                    <d:href>/backups/</d:href>
                    <d:propstat><d:prop><d:resourcetype><d:collection/></d:resourcetype></d:prop></d:propstat>
                  </d:response>
                  <d:response>
                    <d:href>/backups/alpha.zip.enc</d:href>
                    <d:propstat>
                      <d:prop>
                        <d:getcontentlength>100</d:getcontentlength>
                        <d:resourcetype/>
                      </d:prop>
                    </d:propstat>
                  </d:response>
                  <d:response>
                    <d:href>/backups/bravo.zip.enc</d:href>
                    <d:propstat>
                      <d:prop>
                        <d:getcontentlength>200</d:getcontentlength>
                        <d:resourcetype/>
                      </d:prop>
                    </d:propstat>
                  </d:response>
                </d:multistatus>"#,
            )
            .create();
        let p = make_provider(&server.url());
        let list = p.list_remote().unwrap();
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].name, "alpha.zip.enc");
        assert_eq!(list[1].name, "bravo.zip.enc");
        assert_eq!(list[0].size, 100);
    }

    #[test]
    fn test_webdav_provider_401_surfaces_error() {
        let mut server = Server::new();
        let _m = server
            .mock("PROPFIND", "/backups/")
            .with_status(401)
            .with_body("unauthorized")
            .create();
        let p = make_provider(&server.url());
        let err = p.test_connection().unwrap_err();
        assert!(err.to_string().contains("401"), "got: {err}");
    }

    #[test]
    fn test_webdav_provider_test_connection_succeeds() {
        let mut server = Server::new();
        let _m = server
            .mock("PROPFIND", "/backups/")
            .with_status(207)
            .with_header("content-type", "application/xml")
            .with_body(
                r#"<?xml version="1.0" encoding="utf-8"?>
                <d:multistatus xmlns:d="DAV:">
                  <d:response>
                    <d:href>/backups/</d:href>
                    <d:propstat><d:prop><d:resourcetype><d:collection/></d:resourcetype></d:prop></d:propstat>
                  </d:response>
                </d:multistatus>"#,
            )
            .create();
        let p = make_provider(&server.url());
        p.test_connection().unwrap();
    }

    #[test]
    fn test_webdav_provider_full_url_with_and_without_remote_path() {
        let mut p = make_provider("http://x/");
        p.remote_path = "backups".into();
        assert_eq!(p.full_url(""), "http://x/backups/");
        assert_eq!(p.full_url("a.zip"), "http://x/backups/a.zip");
        p.remote_path = "".into();
        assert_eq!(p.full_url(""), "http://x/");
        assert_eq!(p.full_url("a.zip"), "http://x/a.zip");
    }

    #[test]
    fn test_webdav_provider_put_5xx_surfaces_error() {
        let mut server = Server::new();
        let _mk = server
            .mock("MKCOL", "/backups/")
            .with_status(201)
            .create();
        let _m = server
            .mock("PUT", "/backups/bad.zip.enc")
            .with_status(500)
            .with_body("internal error")
            .create();
        let p = make_provider(&server.url());
        let err = p.upload(b"X", "bad.zip.enc").unwrap_err();
        assert!(err.to_string().contains("500"), "got: {err}");
    }

    #[test]
    fn test_parse_propfind_handles_empty() {
        let list = parse_propfind_response("<?xml ?>", "http://x/backups/");
        assert!(list.is_empty());
    }
}
