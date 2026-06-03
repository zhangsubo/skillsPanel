use crate::core::error::AppError;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct ParsedGitSource {
    pub original_url: String,
    pub clone_url: String,
    pub branch: Option<String>,
    pub subpath: Option<String>,
    /// Full path after `tree/` in tree URLs (e.g., `feature/x/skills`).
    /// Used for post-clone resolution when branch name contains slashes.
    pub tree_path: Option<String>,
}

pub struct GitUrlParser;

impl GitUrlParser {
    pub fn validate_git_url(url: &str) -> Result<String, AppError> {
        let trimmed = url.trim();

        if trimmed.starts_with("file://") {
            return Err(AppError::Git(
                "Local file:// URLs are not supported. Use local installation instead.".into(),
            ));
        }

        if Path::new(trimmed).is_absolute() {
            return Err(AppError::Git(
                "Local paths are not supported for Git installation. Use local installation instead.".into(),
            ));
        }

        if trimmed.contains("..") {
            return Err(AppError::Git(
                "Invalid Git URL: path traversal detected".into(),
            ));
        }

        if trimmed.is_empty() {
            return Err(AppError::Git("Git URL cannot be empty".into()));
        }

        Ok(trimmed.to_string())
    }

    pub fn parse_git_source(url: &str) -> ParsedGitSource {
        let original_url = url.trim().to_string();

        // Try tree URL parsing first (before normalization strips tree info)
        if let Some(tree_info) = Self::parse_github_content_url(&original_url) {
            return ParsedGitSource {
                original_url,
                clone_url: tree_info.0,
                branch: Some(tree_info.1),
                subpath: tree_info.2,
                tree_path: tree_info.3,
            };
        }

        let validated = Self::normalize_git_url(&original_url);

        ParsedGitSource {
            original_url: original_url.clone(),
            clone_url: validated,
            branch: None,
            subpath: None,
            tree_path: None,
        }
    }

    pub fn canonicalize_clone_url(url: &str) -> String {
        url.trim()
            .trim_end_matches('/')
            .trim_end_matches(".git")
            .to_string()
    }

    pub fn normalize_git_url(url: &str) -> String {
        let trimmed = url.trim();

        if trimmed.starts_with("https://")
            || trimmed.starts_with("http://")
            || trimmed.starts_with("git@")
            || trimmed.starts_with("ssh://")
        {
            return Self::extract_clean_repo_url(trimmed).unwrap_or_else(|| trimmed.to_string());
        }

        if !trimmed.contains('/') {
            return format!("https://github.com/{}", trimmed);
        }

        let parts: Vec<&str> = trimmed.split('/').collect();
        if parts.len() == 2 && !parts[0].contains('.') {
            return format!("https://github.com/{}", trimmed);
        }

        if trimmed.contains("github.com")
            || trimmed.contains("gitlab.com")
            || trimmed.contains("bitbucket.org")
        {
            return Self::extract_clean_repo_url(trimmed)
                .unwrap_or_else(|| format!("https://{}", trimmed));
        }

        format!("https://{}", trimmed)
    }

    fn extract_clean_repo_url(url: &str) -> Option<String> {
        let patterns = ["github.com/", "gitlab.com/", "bitbucket.org/"];
        for pattern in &patterns {
            if let Some(pos) = url.find(pattern) {
                let after = &url[pos + pattern.len()..];
                let parts: Vec<&str> = after.split('/').collect();
                if parts.len() >= 2 {
                    let owner = parts[0];
                    let repo = parts[1].trim_end_matches(".git");
                    if !owner.is_empty() && !repo.is_empty() {
                        let protocol = if url.starts_with("https://") {
                            "https://"
                        } else if url.starts_with("http://") {
                            "http://"
                        } else {
                            "https://"
                        };
                        return Some(format!("{}{}{}/{}", protocol, pattern, owner, repo));
                    }
                }
            }
        }
        None
    }

    fn parse_github_content_url(
        url: &str,
    ) -> Option<(String, String, Option<String>, Option<String>)> {
        let github_prefix = "https://github.com/";
        if !url.starts_with(github_prefix) {
            return None;
        }
        let after_prefix = &url[github_prefix.len()..];
        let segments: Vec<&str> = after_prefix.split('/').collect();
        if segments.len() < 4 {
            return None;
        }
        let owner = segments[0];
        let repo = segments[1].trim_end_matches(".git");
        if owner.is_empty() || repo.is_empty() {
            return None;
        }
        // Support both /tree/ and /blob/ URL formats
        let url_type = segments[2];
        if url_type != "tree" && url_type != "blob" {
            return None;
        }
        let tree_path = segments[3..].join("/");
        Some((
            format!("https://github.com/{}/{}", owner, repo),
            tree_path.clone(),
            None,
            Some(tree_path),
        ))
    }

    /// Resolve a tree path (e.g., `feature/x/skills`) into a branch and optional subpath.
    ///
    /// Tries branch candidates from longest to shortest prefix by checking if
    /// the branch exists in the repository (local or remote).
    /// Falls back to first segment as branch (current behavior).
    pub fn resolve_tree_path(
        repo: &git2::Repository,
        tree_path: &str,
    ) -> Option<(String, Option<String>)> {
        let segments: Vec<&str> = tree_path.split('/').collect();
        if segments.is_empty() {
            return None;
        }

        // Try candidates from longest to shortest prefix
        for i in (1..=segments.len()).rev() {
            let candidate = segments[..i].join("/");

            // Check remote branches first (refs/remotes/origin/{candidate})
            let remote_ref = format!("refs/remotes/origin/{}", candidate);
            if repo.find_reference(&remote_ref).is_ok() {
                let subpath = if i < segments.len() {
                    Some(segments[i..].join("/"))
                } else {
                    None
                };
                return Some((candidate, subpath));
            }

            // Check local branches (refs/heads/{candidate})
            let local_ref = format!("refs/heads/{}", candidate);
            if repo.find_reference(&local_ref).is_ok() {
                let subpath = if i < segments.len() {
                    Some(segments[i..].join("/"))
                } else {
                    None
                };
                return Some((candidate, subpath));
            }
        }

        // Fallback: first segment as branch
        let subpath = if segments.len() > 1 {
            Some(segments[1..].join("/"))
        } else {
            None
        };
        Some((segments[0].to_string(), subpath))
    }

    pub fn extract_github_repo_url(url: &str) -> Option<String> {
        Self::extract_clean_repo_url(url)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_https_url() {
        let result = GitUrlParser::validate_git_url("https://github.com/user/repo.git");
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_file_url_rejected() {
        let result = GitUrlParser::validate_git_url("file:///home/user/repo");
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_absolute_path_rejected() {
        let result = GitUrlParser::validate_git_url("/home/user/repo");
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_shorthand() {
        let result = GitUrlParser::validate_git_url("user/repo");
        assert!(result.is_ok());
    }

    #[test]
    fn test_parse_https() {
        let parsed = GitUrlParser::parse_git_source("https://github.com/user/repo.git");
        assert_eq!(parsed.clone_url, "https://github.com/user/repo");
        assert!(parsed.branch.is_none());
    }

    #[test]
    fn test_parse_tree_url() {
        let parsed =
            GitUrlParser::parse_git_source("https://github.com/user/repo/tree/main/tools/skill");
        assert_eq!(parsed.clone_url, "https://github.com/user/repo");
        assert_eq!(parsed.branch, Some("main/tools/skill".to_string()));
        assert_eq!(parsed.tree_path, Some("main/tools/skill".to_string()));
        assert!(parsed.subpath.is_none());
    }

    #[test]
    fn test_parse_blob_url() {
        // blob URLs should be treated the same as tree URLs
        let parsed =
            GitUrlParser::parse_git_source("https://github.com/user/repo/blob/main/path/to/skill");
        assert_eq!(parsed.clone_url, "https://github.com/user/repo");
        assert_eq!(parsed.branch, Some("main/path/to/skill".to_string()));
        assert_eq!(parsed.tree_path, Some("main/path/to/skill".to_string()));
        assert!(parsed.subpath.is_none());
    }

    #[test]
    fn test_parse_blob_url_real_example() {
        // Real-world blob URL like the user's case
        let parsed = GitUrlParser::parse_git_source(
            "https://github.com/anthropics/skills/blob/main/skills/skill-creator/",
        );
        assert_eq!(parsed.clone_url, "https://github.com/anthropics/skills");
        assert_eq!(parsed.branch, Some("main/skills/skill-creator".to_string()));
        assert_eq!(
            parsed.tree_path,
            Some("main/skills/skill-creator".to_string())
        );
        assert!(parsed.subpath.is_none());
    }

    #[test]
    fn test_parse_shorthand() {
        let parsed = GitUrlParser::parse_git_source("user/repo");
        assert_eq!(parsed.clone_url, "https://github.com/user/repo");
    }

    #[test]
    fn test_canonicalize_url() {
        let c1 = GitUrlParser::canonicalize_clone_url("https://github.com/user/repo.git");
        let c2 = GitUrlParser::canonicalize_clone_url("https://github.com/user/repo/");
        assert_eq!(c1, c2);
    }

    #[test]
    fn test_tree_url_branch_with_slash() {
        let parsed = GitUrlParser::parse_git_source(
            "https://github.com/user/repo/tree/feature/x/skills/foo",
        );
        assert_eq!(parsed.clone_url, "https://github.com/user/repo");
        assert_eq!(parsed.branch, Some("feature/x/skills/foo".to_string()));
        assert_eq!(parsed.tree_path, Some("feature/x/skills/foo".to_string()));
        assert!(parsed.subpath.is_none());
    }
}
