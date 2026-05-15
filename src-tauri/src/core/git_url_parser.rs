use crate::core::error::AppError;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct ParsedGitSource {
    pub original_url: String,
    pub clone_url: String,
    pub branch: Option<String>,
    pub subpath: Option<String>,
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
            return Err(AppError::Git("Invalid Git URL: path traversal detected".into()));
        }

        if trimmed.is_empty() {
            return Err(AppError::Git("Git URL cannot be empty".into()));
        }

        Ok(trimmed.to_string())
    }

    pub fn parse_git_source(url: &str) -> ParsedGitSource {
        let original_url = url.trim().to_string();

        // Try tree URL parsing first (before normalization strips tree info)
        if let Some(tree_info) = Self::parse_tree_url(&original_url) {
            return ParsedGitSource {
                original_url,
                clone_url: tree_info.0,
                branch: Some(tree_info.1),
                subpath: tree_info.2,
            };
        }

        let validated = Self::normalize_git_url(&original_url);

        ParsedGitSource {
            original_url: original_url.clone(),
            clone_url: validated,
            branch: None,
            subpath: None,
        }
    }

    pub fn canonicalize_clone_url(url: &str) -> String {
        url.trim()
            .trim_end_matches('/')
            .trim_end_matches(".git")
            .to_string()
    }

    fn normalize_git_url(url: &str) -> String {
        let trimmed = url.trim();

        if trimmed.starts_with("https://") || trimmed.starts_with("http://") || trimmed.starts_with("git@") || trimmed.starts_with("ssh://") {
            return Self::extract_clean_repo_url(trimmed).unwrap_or_else(|| trimmed.to_string());
        }

        if !trimmed.contains('/') {
            return format!("https://github.com/{}", trimmed);
        }

        let parts: Vec<&str> = trimmed.split('/').collect();
        if parts.len() == 2 && !parts[0].contains('.') {
            return format!("https://github.com/{}", trimmed);
        }

        if trimmed.contains("github.com") || trimmed.contains("gitlab.com") || trimmed.contains("bitbucket.org") {
            return Self::extract_clean_repo_url(trimmed).unwrap_or_else(|| format!("https://{}", trimmed));
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
                        let protocol = if url.starts_with("https://") { "https://" }
                            else if url.starts_with("http://") { "http://" }
                            else { "https://" };
                        return Some(format!("{}{}{}/{}", protocol, pattern, owner, repo));
                    }
                }
            }
        }
        None
    }

    fn parse_tree_url(url: &str) -> Option<(String, String, Option<String>)> {
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
        if segments[2] != "tree" {
            return None;
        }
        let branch = segments[3].to_string();
        let subpath = if segments.len() > 4 {
            Some(segments[4..].join("/"))
        } else {
            None
        };
        Some((format!("https://github.com/{}/{}", owner, repo), branch, subpath))
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
        let parsed = GitUrlParser::parse_git_source("https://github.com/user/repo/tree/main/tools/skill");
        assert_eq!(parsed.clone_url, "https://github.com/user/repo");
        assert_eq!(parsed.branch, Some("main".to_string()));
        assert_eq!(parsed.subpath, Some("tools/skill".to_string()));
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
        let parsed = GitUrlParser::parse_git_source("https://github.com/user/repo/tree/feature/x/skills/foo");
        assert_eq!(parsed.clone_url, "https://github.com/user/repo");
        assert_eq!(parsed.branch, Some("feature".to_string()));
        assert_eq!(parsed.subpath, Some("x/skills/foo".to_string()));
    }
}