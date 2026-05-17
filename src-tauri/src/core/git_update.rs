use crate::core::database::{Database, SkillsRepository};
use crate::core::error::AppError;
use crate::core::git_clone;
use crate::core::library::SkillLibrary;
use crate::core::models::{Skill, SourceUpdateStatus};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

/// Check if a git-installed skill has an update available.
///
/// Compares the remote HEAD with the stored source_revision.
pub fn check_git_skill_update(skill: &Skill, database: &Database) -> Result<bool, AppError> {
    let git_url = skill
        .original_git_url
        .as_ref()
        .ok_or_else(|| AppError::InvalidSkill("Skill is not a git skill".into()))?;

    let _subpath = skill.original_git_subpath.as_deref();

    // Clone to a temp directory to check remote HEAD
    let temp_dir = tempfile::tempdir()?;
    let clone_root = temp_dir.path().join("repo");

    let parsed = crate::core::git_url_parser::GitUrlParser::parse_git_source(git_url);
    let cancel = Arc::new(AtomicBool::new(false));

    git_clone::clone_with_cache(
        &parsed.clone_url,
        &clone_root,
        parsed.branch.as_deref(),
        Some(cancel),
        None,
    )?;

    let remote_head = git_clone::get_head_sha(&clone_root)?;

    let has_update = match &skill.source_revision {
        Some(local_rev) => local_rev != &remote_head,
        None => true,
    };

    // Update the skill's remote revision and status
    let skills_repo = SkillsRepository::new(database);
    let update_status = if has_update {
        SourceUpdateStatus::UpdateAvailable
    } else {
        SourceUpdateStatus::UpToDate
    };

    skills_repo.update_source_remote_revision(&skill.id, &remote_head, &update_status)?;

    Ok(has_update)
}

/// Update a git-installed skill to the latest version.
///
/// Clones the latest version, copies to library, updates DB.
pub fn update_git_skill(
    skill: &Skill,
    library: &SkillLibrary,
    database: &Database,
    cancel: Option<Arc<AtomicBool>>,
) -> Result<String, AppError> {
    let git_url = skill
        .original_git_url
        .as_ref()
        .ok_or_else(|| AppError::InvalidSkill("Skill is not a git skill".into()))?;

    let subpath = skill.original_git_subpath.clone();

    // Clone latest version
    let temp_dir = tempfile::tempdir()?;
    let clone_root = temp_dir.path().join("repo");

    let parsed = crate::core::git_url_parser::GitUrlParser::parse_git_source(git_url);
    let cancel_token = cancel.unwrap_or_else(|| Arc::new(AtomicBool::new(false)));

    git_clone::clone_with_cache(
        &parsed.clone_url,
        &clone_root,
        parsed.branch.as_deref(),
        Some(cancel_token),
        None,
    )?;

    // Get the skill directory
    let skill_dir = crate::core::fs_utils::resolve_skill_dir(&clone_root, subpath.as_deref())?;

    // Get new HEAD SHA
    let head_sha = git_clone::get_head_sha(&skill_dir)?;

    // Copy to library (overwrite existing)
    let dest = library.add_skill_with_overwrite(&skill_dir, &skill.name)?;

    // Update DB
    let skills_repo = SkillsRepository::new(database);
    skills_repo.update_source_revision(&skill.id, &head_sha)?;

    Ok(dest.to_string_lossy().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_check_update_no_git_url() {
        let skill = Skill {
            id: "test".to_string(),
            name: "test".to_string(),
            path_hash: "hash".to_string(),
            library_path: "/test".to_string(),
            original_source_path: None,
            original_git_url: None,
            original_git_subpath: None,
            group: "default".to_string(),
            description: "test".to_string(),
            frontmatter: std::collections::HashMap::new(),
            created_at: "2024-01-01T00:00:00Z".to_string(),
            mtime_ms: 0,
            source_type: crate::core::models::SkillSourceType::LocalFolder,
            is_deleted: false,
            content_hash: None,
            source_revision: None,
            source_remote_revision: None,
            source_update_status: Default::default(),
        };

        let temp = tempfile::NamedTempFile::new().unwrap();
        let db = Database::new(&temp.path().to_path_buf()).unwrap();

        let result = check_git_skill_update(&skill, &db);
        assert!(result.is_err());
    }
}
