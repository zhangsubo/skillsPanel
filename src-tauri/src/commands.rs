use crate::core::config::AppConfig;
use crate::core::content_hash::ContentHash;
use crate::core::error::AppError;
use crate::core::library::SkillLibrary;
use crate::core::models::*;
use crate::core::repo_lock::RepoLock;
use crate::core::skill_engine::SkillEngine;
use crate::AppState;
use std::sync::Mutex;
use tauri::Emitter;
use tauri::State;

type SharedState = Mutex<AppState>;

fn find_tool<'a>(config: &'a AppConfig, tool_id: &str) -> Result<&'a Tool, AppError> {
    config
        .tools
        .iter()
        .find(|t| t.id == tool_id)
        .ok_or_else(|| AppError::ToolNotFound(tool_id.to_string()))
}

#[tauri::command]
pub fn get_config(state: State<'_, SharedState>) -> Result<String, AppError> {
    let state = state.lock().unwrap();
    let config = state.config.lock().unwrap();
    serde_json::to_string(&*config).map_err(|e| AppError::Config(e.to_string()))
}

#[tauri::command]
pub fn update_config(state: State<'_, SharedState>, config_json: String) -> Result<(), AppError> {
    let state = state.lock().unwrap();
    let mut config = state.config.lock().unwrap();
    let database = state.database.lock().unwrap();
    let new_config: AppConfig =
        serde_json::from_str(&config_json).map_err(|e| AppError::Config(e.to_string()))?;

    let db_repo = crate::core::database::ConfigRepository::new(&database);
    let tools_json = serde_json::to_string(&new_config.tools)
        .map_err(|e| AppError::Config(format!("Failed to serialize tools: {}", e)))?;
    let sources_json = serde_json::to_string(&new_config.sources)
        .map_err(|e| AppError::Config(format!("Failed to serialize sources: {}", e)))?;
    let rules_json = serde_json::to_string(&new_config.rules)
        .map_err(|e| AppError::Config(format!("Failed to serialize rules: {}", e)))?;
    let sync_json = serde_json::to_string(&new_config.sync)
        .map_err(|e| AppError::Config(format!("Failed to serialize sync: {}", e)))?;
    let install_json = serde_json::to_string(&new_config.install)
        .map_err(|e| AppError::Config(format!("Failed to serialize install: {}", e)))?;
    let exclude_json = serde_json::to_string(&new_config.exclude_paths)
        .map_err(|e| AppError::Config(format!("Failed to serialize exclude_paths: {}", e)))?;
    let deleted_json = serde_json::to_string(&new_config.deleted_skills)
        .map_err(|e| AppError::Config(format!("Failed to serialize deleted_skills: {}", e)))?;

    db_repo.set("tools", &tools_json)?;
    db_repo.set("sources", &sources_json)?;
    db_repo.set("rules", &rules_json)?;
    db_repo.set("sync", &sync_json)?;
    db_repo.set("install", &install_json)?;
    db_repo.set("exclude_paths", &exclude_json)?;
    db_repo.set("deleted_skills", &deleted_json)?;

    let _ = new_config.save();
    *config = new_config;
    Ok(())
}

#[tauri::command]
pub fn get_tools(state: State<'_, SharedState>) -> Result<Vec<Tool>, AppError> {
    let state = state.lock().unwrap();
    let config = state.config.lock().unwrap();
    Ok(config.tools.clone())
}

#[tauri::command]
pub fn get_library(state: State<'_, SharedState>) -> Result<Vec<String>, AppError> {
    let state = state.lock().unwrap();
    let library = state.library.lock().unwrap();
    library.list_skills()
}

#[tauri::command]
pub fn scan_skills(state: State<'_, SharedState>) -> Result<ScanResult, AppError> {
    let state = state.lock().unwrap();
    let config = state.config.lock().unwrap();
    let library = state.library.lock().unwrap();
    let database = state.database.lock().unwrap();

    let scan_timestamp = chrono::Utc::now().to_rfc3339();
    let skills = crate::core::scanner::Scanner::scan_sources(&config, &library)?;

    let skills_repo = crate::core::database::SkillsRepository::new(&database);
    for skill_with_status in &skills {
        skills_repo.upsert_with_scan(&skill_with_status.skill, &scan_timestamp)?;
    }
    skills_repo.mark_missing_as_deleted(&scan_timestamp)?;

    let total_skills = skills.len();
    let total_tools = config.tools.len();
    let mut linked_count = 0;
    let mut conflict_count = 0;
    let mut blocked_count = 0;

    for skill in &skills {
        for (_tool_name, status) in &skill.tool_statuses {
            match status {
                SkillToolStatus::Linked => linked_count += 1,
                SkillToolStatus::Wrong | SkillToolStatus::Directory => conflict_count += 1,
                SkillToolStatus::Blocked => blocked_count += 1,
                _ => {}
            }
        }
    }

    for skill_with_status in &skills {
        if let Ok(Some(existing)) = skills_repo.get_by_name(&skill_with_status.skill.name) {
            if existing.description != skill_with_status.skill.description {
                let _ = skills_repo.update_description(
                    &skill_with_status.skill.name,
                    &skill_with_status.skill.description,
                );
            }
        }
    }

    Ok(ScanResult {
        total_skills,
        total_tools,
        linked_count,
        conflict_count,
        blocked_count,
        skills,
    })
}

#[tauri::command]
pub fn get_scan_diff(
    state: State<'_, SharedState>,
) -> Result<crate::core::scan_db::ScanDiff, AppError> {
    let state = state.lock().unwrap();
    let database = state.database.lock().unwrap();

    let scan_timestamp = chrono::Utc::now().to_rfc3339();
    let skills_repo = crate::core::database::SkillsRepository::new(&database);

    let added_skills = skills_repo.get_new_skills(&scan_timestamp)?;
    let updated_skills = skills_repo.get_updated_skills(&scan_timestamp)?;
    let deleted_skills = skills_repo.get_deleted_skills()?;

    let to_status = |skills: Vec<Skill>| -> Vec<SkillWithStatus> {
        skills
            .into_iter()
            .map(|skill| SkillWithStatus {
                skill,
                tool_statuses: std::collections::HashMap::new(),
                rule_decisions: std::collections::HashMap::new(),
            })
            .collect()
    };

    Ok(crate::core::scan_db::ScanDiff {
        added: to_status(added_skills),
        updated: to_status(updated_skills),
        deleted: to_status(deleted_skills),
    })
}

#[tauri::command]
pub fn get_skill_content(
    state: State<'_, SharedState>,
    skill_id: String,
) -> Result<String, AppError> {
    let state = state.lock().unwrap();
    let library = state.library.lock().unwrap();
    let skill_path = library.skill_path(&skill_id);

    let skill_md = crate::core::fs_utils::find_skill_marker(&skill_path).ok_or_else(|| {
        AppError::SkillNotFound(format!(
            "{} (no SKILL.md/skill.md in {})",
            skill_id,
            skill_path.display()
        ))
    })?;

    std::fs::read_to_string(&skill_md).map_err(AppError::Io)
}

#[tauri::command]
pub fn preview_local_install(path: String) -> Result<Vec<InstallCandidate>, AppError> {
    crate::core::scanner::Scanner::preview_local_install(std::path::Path::new(&path))
}

#[tauri::command]
pub fn preview_git_install(
    git_url: String,
    subpath: Option<String>,
) -> Result<Vec<InstallCandidate>, AppError> {
    crate::core::scanner::Scanner::preview_git_install(&git_url, subpath.as_deref())
}

#[tauri::command]
pub fn install_local_skill(
    state: State<'_, SharedState>,
    window: tauri::Window,
    source_path: String,
    name: Option<String>,
) -> Result<String, AppError> {
    let _lock = RepoLock::acquire("install local skill")?;

    let _ = window.emit(
        "install-progress",
        serde_json::json!({
            "stage": "installing",
            "message": format!("Installing from {}...", source_path),
        }),
    );

    let state = state.lock().unwrap();
    let library = state.library.lock().unwrap();
    let database = state.database.lock().unwrap();

    let path = std::path::Path::new(&source_path);
    let source = if path.is_file() && SkillEngine::is_zip_file(path) {
        crate::core::skill_engine::SkillSource::Zip(path.to_path_buf())
    } else {
        crate::core::skill_engine::SkillSource::Folder(path.to_path_buf())
    };

    let result = SkillEngine::install_skill(source, &library, &database, name)?;

    let hash = ContentHash::hash_directory(&result.library_path).unwrap_or_default();
    if !hash.is_empty() {
        let skills_repo = crate::core::database::SkillsRepository::new(&database);
        let _ = skills_repo.update_content_hash(&result.skill_id, &hash);
    }

    let _ = window.emit(
        "install-progress",
        serde_json::json!({
            "stage": "complete",
            "message": "Installation complete",
        }),
    );

    Ok(result.library_path.to_string_lossy().to_string())
}

#[tauri::command]
pub fn install_git_skill(
    state: State<'_, SharedState>,
    window: tauri::Window,
    git_url: String,
    subpath: Option<String>,
    name: Option<String>,
) -> Result<String, AppError> {
    let _lock = RepoLock::acquire("install git skill")?;

    let state = state.lock().unwrap();
    let cancel_registry = state.cancel_registry.clone();

    // Register cancellation
    let install_key = format!("git-{}", git_url);
    let cancel_token = cancel_registry.register(&install_key);
    let _guard = crate::core::install_cancel::CancelRegistrationGuard::new(
        cancel_registry.clone(),
        install_key.clone(),
    );

    let window_clone = window.clone();
    let progress_fn: std::sync::Arc<dyn Fn(&crate::core::git_clone::CloneProgress) + Send + Sync> =
        std::sync::Arc::new(move |progress| {
            let _ = window_clone.emit(
                "install-progress",
                serde_json::json!({
                    "stage": progress.stage,
                    "message": progress.message,
                }),
            );
        });

    let _ = window.emit(
        "install-progress",
        serde_json::json!({
            "stage": "cloning",
            "message": format!("Cloning {}...", git_url),
        }),
    );

    let library = state.library.lock().unwrap();
    let database = state.database.lock().unwrap();

    // When no subpath specified, try installing all skills at once
    if subpath.is_none() {
        match SkillEngine::install_all_git_skills(
            &git_url,
            &library,
            &database,
            Some(cancel_token.clone()),
            Some(progress_fn.clone()),
            name.as_deref(),
        ) {
            Ok(results) => {
                let _ = window.emit(
                    "install-progress",
                    serde_json::json!({
                        "stage": "hashing",
                        "message": "Computing content hashes...",
                    }),
                );

                let skills_repo = crate::core::database::SkillsRepository::new(&database);
                for result in &results {
                    let hash =
                        ContentHash::hash_directory(&result.library_path).unwrap_or_default();
                    if !hash.is_empty() {
                        let _ = skills_repo.update_content_hash(&result.skill_id, &hash);
                    }
                    if let Some(ref head_sha) = result.head_sha {
                        let _ = skills_repo.update_source_revision(&result.skill_id, head_sha);
                    }
                }

                let _ = window.emit(
                    "install-progress",
                    serde_json::json!({
                        "stage": "complete",
                        "message": format!("Installed {} skills", results.len()),
                    }),
                );

                // Return comma-separated paths
                let paths: Vec<String> = results
                    .iter()
                    .map(|r| r.library_path.to_string_lossy().to_string())
                    .collect();
                return Ok(paths.join(","));
            }
            Err(e) => {
                eprintln!("[install_git_skill] install_all_git_skills failed: {}. Falling back to single-skill install.", e);
                // Fall through to single-skill install below
            }
        }
    }

    let source = crate::core::skill_engine::SkillSource::Git {
        url: git_url,
        subpath,
    };

    let result = SkillEngine::install_skill_with_progress(
        source,
        &library,
        &database,
        name,
        Some(cancel_token),
        Some(progress_fn),
    )?;

    let _ = window.emit(
        "install-progress",
        serde_json::json!({
            "stage": "hashing",
            "message": "Computing content hash...",
        }),
    );

    let hash = ContentHash::hash_directory(&result.library_path).unwrap_or_default();
    if !hash.is_empty() {
        let skills_repo = crate::core::database::SkillsRepository::new(&database);
        let _ = skills_repo.update_content_hash(&result.skill_id, &hash);
    }

    // Update source_remote_revision if we got a HEAD SHA
    if let Some(ref head_sha) = result.head_sha {
        let skills_repo = crate::core::database::SkillsRepository::new(&database);
        let _ = skills_repo.update_source_revision(&result.skill_id, head_sha);
    }

    let _ = window.emit(
        "install-progress",
        serde_json::json!({
            "stage": "complete",
            "message": "Installation complete",
        }),
    );

    Ok(result.library_path.to_string_lossy().to_string())
}

#[tauri::command]
pub fn link_skill(
    state: State<'_, SharedState>,
    skill_name: String,
    tool_id: String,
) -> Result<(), AppError> {
    let state = state.lock().unwrap();
    let library = state.library.lock().unwrap();
    let config = state.config.lock().unwrap();
    let database = state.database.lock().unwrap();
    let tool = find_tool(&config, &tool_id)?;
    let tool_dir = tool.expanded_path();
    let skill_path = library.skill_path(&skill_name);

    // Resolve the skill id once so the DB/FS branches below can share it.
    let skill_db_repo = crate::core::database::SkillsRepository::new(&database);
    let skill_id_opt = skill_db_repo
        .get_skill_id_by_name(&skill_name)
        .ok()
        .flatten();

    // Self-heal: if the DB already marks this (tool, skill) as active but
    // the symlink on disk is gone (e.g. removed externally by the tool's
    // own startup), recreate it before going through the normal path. This
    // is the common case that previously returned `OK` while the UI still
    // saw the skill as unlinked (because the scanner later reported
    // `Missing` for the same key — see scanner.rs path resolution fix).
    if let Some(ref skill_id) = skill_id_opt {
        let links_repo = crate::core::database::LinksRepository::new(&database);
        if links_repo.is_active(&tool_id, skill_id)? {
            let target = tool_dir.join(&skill_name);
            let symlink_present = target
                .symlink_metadata()
                .map(|m| m.file_type().is_symlink())
                .unwrap_or(false);
            if !symlink_present {
                crate::core::linker::Linker::fix_link(&skill_path, &tool_dir, &skill_name)?;
                let final_status =
                    crate::core::linker::Linker::check_status(&skill_path, &tool_dir, &skill_name);
                if !matches!(final_status, SkillToolStatus::Linked) {
                    return Err(AppError::Link(format!(
                        "self-heal failed: symlink at '{}' is not linked to '{}'",
                        target.display(),
                        skill_path.display()
                    )));
                }
                return Ok(());
            }
        }
    }

    crate::core::linker::Linker::link(&skill_path, &tool_dir, &skill_name)?;

    if let Some(skill_id) = skill_id_opt {
        let links_repo = crate::core::database::LinksRepository::new(&database);
        let _ = links_repo.link(&tool_id, &skill_id);
    }

    Ok(())
}

#[tauri::command]
pub fn unlink_skill(
    state: State<'_, SharedState>,
    skill_name: String,
    tool_id: String,
) -> Result<(), AppError> {
    let state = state.lock().unwrap();
    let config = state.config.lock().unwrap();
    let database = state.database.lock().unwrap();
    let tool = find_tool(&config, &tool_id)?;
    crate::core::linker::Linker::unlink(&tool.expanded_path(), &skill_name)?;

    let skill_db_repo = crate::core::database::SkillsRepository::new(&database);
    if let Ok(Some(skill_id)) = skill_db_repo.get_skill_id_by_name(&skill_name) {
        let links_repo = crate::core::database::LinksRepository::new(&database);
        let _ = links_repo.unlink(&tool_id, &skill_id);
    }

    Ok(())
}

#[tauri::command]
pub fn fix_skill_link(
    state: State<'_, SharedState>,
    skill_name: String,
    tool_id: String,
) -> Result<(), AppError> {
    let state = state.lock().unwrap();
    let library = state.library.lock().unwrap();
    let config = state.config.lock().unwrap();
    let tool = find_tool(&config, &tool_id)?;
    let skill_path = library.skill_path(&skill_name);
    crate::core::linker::Linker::fix_link(&skill_path, &tool.expanded_path(), &skill_name)
}

#[tauri::command]
pub fn update_skill_rule(
    state: State<'_, SharedState>,
    skill_name: String,
    rule: SkillRule,
) -> Result<(), AppError> {
    let state = state.lock().unwrap();
    let mut config = state.config.lock().unwrap();
    config.rules.skills.insert(skill_name, rule);
    config.save()
}

#[tauri::command]
pub fn update_group_rule(
    state: State<'_, SharedState>,
    group: String,
    rule: GroupRule,
) -> Result<(), AppError> {
    let state = state.lock().unwrap();
    let mut config = state.config.lock().unwrap();
    config.rules.groups.insert(group, rule);
    config.save()
}

#[tauri::command]
pub fn update_tool_rule(
    state: State<'_, SharedState>,
    tool_id: String,
    rule: ToolRule,
) -> Result<(), AppError> {
    let state = state.lock().unwrap();
    let mut config = state.config.lock().unwrap();
    config.rules.tools.insert(tool_id, rule);
    config.save()
}

#[tauri::command]
pub fn batch_link_skills(
    state: State<'_, SharedState>,
    skill_names: Vec<String>,
    tool_id: String,
) -> Result<usize, AppError> {
    let state = state.lock().unwrap();
    let library = state.library.lock().unwrap();
    let config = state.config.lock().unwrap();
    let _resolver = crate::core::resolver::Resolver::new(config.rules.clone());
    let tool = find_tool(&config, &tool_id)?;
    let mut count = 0;
    for skill_name in skill_names {
        let skill_path = library.skill_path(&skill_name);
        if skill_path.exists() {
            if let Ok(_) =
                crate::core::linker::Linker::link(&skill_path, &tool.expanded_path(), &skill_name)
            {
                count += 1;
            }
        }
    }
    Ok(count)
}

#[tauri::command]
pub fn batch_unlink_skills(
    state: State<'_, SharedState>,
    skill_names: Vec<String>,
    tool_id: String,
) -> Result<usize, AppError> {
    let state = state.lock().unwrap();
    let config = state.config.lock().unwrap();
    let tool = find_tool(&config, &tool_id)?;
    let mut count = 0;
    for skill_name in skill_names {
        if let Ok(_) = crate::core::linker::Linker::unlink(&tool.expanded_path(), &skill_name) {
            count += 1;
        }
    }
    Ok(count)
}

#[tauri::command]
pub fn batch_export_skills(
    state: State<'_, SharedState>,
    skill_names: Vec<String>,
    target_path: String,
    as_zip: bool,
) -> Result<usize, AppError> {
    let state = state.lock().unwrap();
    let library = state.library.lock().unwrap();
    let mut skills = Vec::new();
    for name in &skill_names {
        let skill_path = library.skill_path(name);
        if skill_path.exists() {
            skills.push(Skill {
                id: format!("{}-{}", name, SkillLibrary::compute_path_hash(&skill_path)),
                name: name.clone(),
                path_hash: SkillLibrary::compute_path_hash(&skill_path),
                library_path: skill_path.to_string_lossy().to_string(),
                original_source_path: None,
                original_git_url: None,
                original_git_subpath: None,
                group: String::new(),
                description: String::new(),
                frontmatter: std::collections::HashMap::new(),
                created_at: String::new(),
                mtime_ms: 0,
                source_type: SkillSourceType::LocalFolder,
                is_deleted: false,
                content_hash: None,
                source_revision: None,
                source_remote_revision: None,
                source_update_status: Default::default(),
            });
        }
    }
    let count = skills.len();
    if as_zip {
        crate::core::exporter::Exporter::export_to_zip(
            &skills,
            std::path::Path::new(&target_path),
        )?;
    } else {
        for skill in &skills {
            crate::core::exporter::Exporter::export_to_folder(
                skill,
                std::path::Path::new(&target_path),
            )?;
        }
    }
    Ok(count)
}

#[tauri::command]
pub fn batch_delete_skills(
    state: State<'_, SharedState>,
    skill_names: Vec<String>,
    _delete_symlinks: bool,
) -> Result<usize, AppError> {
    let state = state.lock().unwrap();
    let library = state.library.lock().unwrap();
    let config = state.config.lock().unwrap();
    let database = state.database.lock().unwrap();
    let db_repo = crate::core::database::SkillsRepository::new(&database);
    let mut count = 0;
    for name in &skill_names {
        for tool in &config.tools {
            let _ = crate::core::linker::Linker::unlink(&tool.expanded_path(), name);
        }
        if library.remove_skill(name).is_ok() {
            let _ = db_repo.delete_by_name(name);
            count += 1;
        }
    }
    Ok(count)
}

#[tauri::command]
pub fn sync_skills(
    state: State<'_, SharedState>,
    skill_names: Option<Vec<String>>,
) -> Result<usize, AppError> {
    let state = state.lock().unwrap();
    let library = state.library.lock().unwrap();
    let config = state.config.lock().unwrap();
    let resolver = crate::core::resolver::Resolver::new(config.rules.clone());
    let names = skill_names.unwrap_or_else(|| library.list_skills().unwrap_or_default());
    let mut count = 0;
    for name in names {
        let skill_path = library.skill_path(&name);
        if !skill_path.exists() {
            continue;
        }
        for tool in &config.tools {
            if !tool.enabled {
                continue;
            }
            if resolver.is_skill_allowed(&create_minimal_skill(&name, &skill_path), &tool.id) {
                if let Ok(_) =
                    crate::core::linker::Linker::link(&skill_path, &tool.expanded_path(), &name)
                {
                    count += 1;
                }
            }
        }
    }
    Ok(count)
}

#[tauri::command]
pub fn clean_stale_links(state: State<'_, SharedState>) -> Result<usize, AppError> {
    let state = state.lock().unwrap();
    let config = state.config.lock().unwrap();
    let mut total = 0;
    for tool in &config.tools {
        let cleaned = crate::core::linker::Linker::clean_stale(&tool.expanded_path())?;
        total += cleaned.len();
    }
    Ok(total)
}

#[tauri::command]
pub fn delete_skill(
    state: State<'_, SharedState>,
    skill_name: String,
    _hard_delete: bool,
) -> Result<(), AppError> {
    let state = state.lock().unwrap();
    let library = state.library.lock().unwrap();
    let config = state.config.lock().unwrap();
    let database = state.database.lock().unwrap();

    for tool in &config.tools {
        let _ = crate::core::linker::Linker::unlink(&tool.expanded_path(), &skill_name);
    }

    library.remove_skill(&skill_name)?;

    let repo = crate::core::database::SkillsRepository::new(&database);
    repo.delete_by_name(&skill_name)?;
    Ok(())
}

#[tauri::command]
pub fn restore_skill(_state: State<'_, SharedState>, _skill_name: String) -> Result<(), AppError> {
    Err(AppError::Validation("Restore not implemented yet".into()))
}

#[tauri::command]
pub fn export_skill(
    state: State<'_, SharedState>,
    skill_name: String,
    target_path: String,
    as_zip: bool,
) -> Result<(), AppError> {
    let state = state.lock().unwrap();
    let library = state.library.lock().unwrap();
    let skill_path = library.skill_path(&skill_name);
    if !skill_path.exists() {
        return Err(AppError::SkillNotFound(skill_name.clone()));
    }
    let skill = create_minimal_skill(&skill_name, &skill_path);
    if as_zip {
        crate::core::exporter::Exporter::export_to_zip(
            &[skill],
            std::path::Path::new(&target_path),
        )?;
    } else {
        crate::core::exporter::Exporter::export_to_folder(
            &skill,
            std::path::Path::new(&target_path),
        )?;
    }
    Ok(())
}

#[tauri::command]
pub fn get_audit_logs(
    state: State<'_, SharedState>,
    limit: Option<usize>,
) -> Result<Vec<AuditEntry>, AppError> {
    let state = state.lock().unwrap();
    let database = state.database.lock().unwrap();
    let repo = crate::core::database::AuditRepository::new(&database);
    repo.get_logs(limit.unwrap_or(100))
}

#[tauri::command]
pub fn log_message(
    state: State<'_, SharedState>,
    level: String,
    message: String,
    source: String,
) -> Result<(), AppError> {
    let state = state.lock().unwrap();
    let database = state.database.lock().unwrap();
    let repo = crate::core::database::AppLogsRepository::new(&database);
    repo.log(&level, &message, &source)?;

    // Write to desktop log file if debug_logging is enabled
    let config = state.config.lock().unwrap();
    if config.debug_logging {
        if let Some(desktop) = dirs::desktop_dir() {
            let today = chrono::Utc::now().format("%Y%m%d").to_string();
            let log_path = desktop.join(format!("skills-panel-{}.txt", today));
            let timestamp = chrono::Utc::now().to_rfc3339();
            let line = format!("[{}] [{}] [{}] {}\n", timestamp, level, source, message);
            let _ = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(log_path)
                .and_then(|mut f| std::io::Write::write_all(&mut f, line.as_bytes()));
        }
    }

    Ok(())
}

#[tauri::command]
pub fn get_app_logs(
    state: State<'_, SharedState>,
    limit: Option<usize>,
) -> Result<Vec<crate::core::models::LogEntry>, AppError> {
    let state = state.lock().unwrap();
    let database = state.database.lock().unwrap();
    let repo = crate::core::database::AppLogsRepository::new(&database);
    repo.get_logs(limit.unwrap_or(100))
}

fn create_minimal_skill(name: &str, path: &std::path::Path) -> Skill {
    Skill {
        id: SkillLibrary::compute_skill_id(name, path),
        name: name.to_string(),
        path_hash: SkillLibrary::compute_path_hash(path),
        library_path: path.to_string_lossy().to_string(),
        original_source_path: None,
        original_git_url: None,
        original_git_subpath: None,
        group: String::new(),
        description: String::new(),
        frontmatter: std::collections::HashMap::new(),
        created_at: String::new(),
        mtime_ms: 0,
        source_type: SkillSourceType::LocalFolder,
        is_deleted: false,
        content_hash: None,
        source_revision: None,
        source_remote_revision: None,
        source_update_status: Default::default(),
    }
}

#[tauri::command]
pub fn get_installed_skills_from_db(state: State<'_, SharedState>) -> Result<Vec<Skill>, AppError> {
    let state = state.lock().unwrap();
    let database = state.database.lock().unwrap();
    let repo = crate::core::database::SkillsRepository::new(&database);
    repo.get_installed()
}

#[tauri::command]
pub fn get_all_active_skills_from_db(
    state: State<'_, SharedState>,
) -> Result<Vec<Skill>, AppError> {
    let state = state.lock().unwrap();
    let database = state.database.lock().unwrap();
    let repo = crate::core::database::SkillsRepository::new(&database);
    repo.get_all_active()
}

#[tauri::command]
pub fn mark_skill_installed(
    state: State<'_, SharedState>,
    skill_id: String,
) -> Result<(), AppError> {
    let state = state.lock().unwrap();
    let database = state.database.lock().unwrap();
    let repo = crate::core::database::SkillsRepository::new(&database);
    repo.mark_installed(&skill_id)
}

#[tauri::command]
pub fn mark_skill_uninstalled(
    state: State<'_, SharedState>,
    skill_name: String,
) -> Result<(), AppError> {
    let state = state.lock().unwrap();
    let database = state.database.lock().unwrap();
    let repo = crate::core::database::SkillsRepository::new(&database);
    repo.mark_uninstalled(&skill_name)
}

#[tauri::command]
pub fn upsert_skill_to_db(state: State<'_, SharedState>, skill: Skill) -> Result<(), AppError> {
    let state = state.lock().unwrap();
    let database = state.database.lock().unwrap();
    let repo = crate::core::database::SkillsRepository::new(&database);
    repo.upsert(&skill)
}

#[tauri::command]
pub fn get_config_value(
    state: State<'_, SharedState>,
    key: String,
) -> Result<Option<String>, AppError> {
    let state = state.lock().unwrap();
    let database = state.database.lock().unwrap();
    let repo = crate::core::database::ConfigRepository::new(&database);
    repo.get(&key)
}

#[tauri::command]
pub fn set_config_value(
    state: State<'_, SharedState>,
    key: String,
    value: String,
) -> Result<(), AppError> {
    let state = state.lock().unwrap();
    let database = state.database.lock().unwrap();
    let repo = crate::core::database::ConfigRepository::new(&database);
    repo.set(&key, &value)
}

#[tauri::command]
pub fn get_audit_logs_from_db(
    state: State<'_, SharedState>,
    limit: Option<usize>,
) -> Result<Vec<AuditEntry>, AppError> {
    let state = state.lock().unwrap();
    let database = state.database.lock().unwrap();
    let repo = crate::core::database::AuditRepository::new(&database);
    repo.get_logs(limit.unwrap_or(100))
}

#[tauri::command]
pub fn log_audit_entry(
    state: State<'_, SharedState>,
    action: String,
    target: String,
    details: Option<String>,
    success: bool,
    error: Option<String>,
) -> Result<(), AppError> {
    let state = state.lock().unwrap();
    let database = state.database.lock().unwrap();
    let repo = crate::core::database::AuditRepository::new(&database);
    repo.log(&action, &target, details, success, error)
}

#[tauri::command]
pub fn get_app_logs_from_db(
    state: State<'_, SharedState>,
    limit: Option<usize>,
) -> Result<Vec<crate::core::models::LogEntry>, AppError> {
    let state = state.lock().unwrap();
    let database = state.database.lock().unwrap();
    let repo = crate::core::database::AppLogsRepository::new(&database);
    repo.get_logs(limit.unwrap_or(100))
}

#[tauri::command]
pub fn log_app_message(
    state: State<'_, SharedState>,
    level: String,
    message: String,
    source: String,
) -> Result<(), AppError> {
    let state = state.lock().unwrap();
    let database = state.database.lock().unwrap();
    let repo = crate::core::database::AppLogsRepository::new(&database);
    repo.log(&level, &message, &source)
}

#[tauri::command]
pub fn get_tools_from_db(state: State<'_, SharedState>) -> Result<Vec<Tool>, AppError> {
    let state = state.lock().unwrap();
    let database = state.database.lock().unwrap();
    let repo = crate::core::database::ToolsRepository::new(&database);
    repo.get_all()
}

#[tauri::command]
pub fn upsert_tool_to_db(state: State<'_, SharedState>, tool: Tool) -> Result<(), AppError> {
    let state = state.lock().unwrap();
    let database = state.database.lock().unwrap();
    let repo = crate::core::database::ToolsRepository::new(&database);
    repo.upsert(&tool)
}

#[tauri::command]
pub fn link_tool_skill_in_db(
    state: State<'_, SharedState>,
    tool_id: String,
    skill_id: String,
) -> Result<(), AppError> {
    let state = state.lock().unwrap();
    let database = state.database.lock().unwrap();
    let repo = crate::core::database::LinksRepository::new(&database);
    repo.link(&tool_id, &skill_id)
}

#[tauri::command]
pub fn unlink_tool_skill_in_db(
    state: State<'_, SharedState>,
    tool_id: String,
    skill_id: String,
) -> Result<(), AppError> {
    let state = state.lock().unwrap();
    let database = state.database.lock().unwrap();
    let repo = crate::core::database::LinksRepository::new(&database);
    repo.unlink(&tool_id, &skill_id)
}

#[tauri::command]
pub fn get_linked_tool_ids(
    state: State<'_, SharedState>,
    skill_id: String,
) -> Result<Vec<String>, AppError> {
    let state = state.lock().unwrap();
    let database = state.database.lock().unwrap();
    let repo = crate::core::database::LinksRepository::new(&database);
    repo.get_linked_tool_ids(&skill_id)
}

#[tauri::command]
pub fn cancel_install(state: State<'_, SharedState>, key: String) -> Result<bool, AppError> {
    let state = state.lock().unwrap();
    Ok(state.cancel_registry.cancel(&key))
}

#[tauri::command]
pub fn check_skill_update(
    state: State<'_, SharedState>,
    skill_id: String,
) -> Result<bool, AppError> {
    let state = state.lock().unwrap();
    let database = state.database.lock().unwrap();

    let skills_repo = crate::core::database::SkillsRepository::new(&database);
    let skill = skills_repo
        .get_by_id(&skill_id)?
        .ok_or_else(|| AppError::SkillNotFound(skill_id))?;

    crate::core::git_update::check_git_skill_update(&skill, &database)
}

#[tauri::command]
pub fn update_skill(state: State<'_, SharedState>, skill_id: String) -> Result<String, AppError> {
    let state = state.lock().unwrap();
    let library = state.library.lock().unwrap();
    let database = state.database.lock().unwrap();

    let skills_repo = crate::core::database::SkillsRepository::new(&database);
    let skill = skills_repo
        .get_by_id(&skill_id)?
        .ok_or_else(|| AppError::SkillNotFound(skill_id))?;

    crate::core::git_update::update_git_skill(&skill, &library, &database, None)
}

// ── Project / Workspace Commands ─────────────────────────────────────

#[tauri::command]
pub fn create_project(
    state: State<'_, SharedState>,
    name: String,
    root_path: String,
) -> Result<String, AppError> {
    let state = state.lock().unwrap();
    let database = state.database.lock().unwrap();
    let repo = crate::core::database::ProjectsRepository::new(&database);
    let id = uuid::Uuid::new_v4().to_string();
    repo.create(&id, &name, &root_path)?;
    Ok(id)
}

#[tauri::command]
pub fn list_projects(
    state: State<'_, SharedState>,
) -> Result<Vec<crate::core::models::Project>, AppError> {
    let state = state.lock().unwrap();
    let database = state.database.lock().unwrap();
    let repo = crate::core::database::ProjectsRepository::new(&database);
    repo.get_all()
}

#[tauri::command]
pub fn delete_project(state: State<'_, SharedState>, project_id: String) -> Result<(), AppError> {
    let state = state.lock().unwrap();
    let database = state.database.lock().unwrap();
    let repo = crate::core::database::ProjectsRepository::new(&database);
    repo.delete(&project_id)
}

#[tauri::command]
pub fn scan_project(
    state: State<'_, SharedState>,
    project_id: String,
) -> Result<crate::core::models::ProjectDto, AppError> {
    let state = state.lock().unwrap();
    let database = state.database.lock().unwrap();

    let projects_repo = crate::core::database::ProjectsRepository::new(&database);
    let project = projects_repo
        .get_by_id(&project_id)?
        .ok_or_else(|| AppError::SkillNotFound(format!("Project not found: {}", project_id)))?;

    // 两阶段扫描：phase1 立即收集元信息（不进任何 IO 锁），phase2 集中算 hash。
    // 根因 1（性能瓶颈）+ 根因 4（_library 无意义锁）一起修。
    let mut skills = crate::core::project_scanner::ProjectScanner::scan_project_skills_phase1(
        &project.root_path,
    )?;
    crate::core::project_scanner::ProjectScanner::compute_all_hashes(&mut skills)?;

    let skills_repo = crate::core::database::SkillsRepository::new(&database);
    let center_skills = skills_repo.get_all_active()?;

    crate::core::project_scanner::ProjectScanner::classify_sync_status(&mut skills, &center_skills);
    let sync_health = crate::core::project_scanner::ProjectScanner::compute_sync_health(&skills);

    Ok(crate::core::models::ProjectDto {
        project,
        skills,
        sync_health,
    })
}

#[tauri::command]
pub fn import_project_skill(
    state: State<'_, SharedState>,
    project_id: String,
    skill_name: String,
) -> Result<String, AppError> {
    let state = state.lock().unwrap();
    let database = state.database.lock().unwrap();
    let library = state.library.lock().unwrap();

    let projects_repo = crate::core::database::ProjectsRepository::new(&database);
    let project = projects_repo
        .get_by_id(&project_id)?
        .ok_or_else(|| AppError::SkillNotFound(format!("Project not found: {}", project_id)))?;

    crate::core::project_scanner::ProjectScanner::import_project_skill_to_center(
        &database,
        &project.root_path,
        &skill_name,
        &library,
    )
}

#[tauri::command]
pub fn export_skill_to_project(
    state: State<'_, SharedState>,
    project_id: String,
    skill_name: String,
    agent: String,
) -> Result<(), AppError> {
    let state = state.lock().unwrap();
    let database = state.database.lock().unwrap();
    let library = state.library.lock().unwrap();

    let projects_repo = crate::core::database::ProjectsRepository::new(&database);
    let project = projects_repo
        .get_by_id(&project_id)?
        .ok_or_else(|| AppError::SkillNotFound(format!("Project not found: {}", project_id)))?;

    crate::core::project_scanner::ProjectScanner::export_center_skill_to_project(
        &database,
        &skill_name,
        &project.root_path,
        &agent,
        &library,
    )
}

#[tauri::command]
pub fn get_app_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::config::AppConfig;
    use crate::core::database::Database;
    use crate::core::library::SkillLibrary;
    use std::fs;
    use std::io::Write;
    use std::path::Path;
    use tempfile::TempDir;
    use zip::write::SimpleFileOptions;
    use zip::CompressionMethod;
    use zip::ZipWriter;

    fn create_test_config(library_path: &Path) -> AppConfig {
        AppConfig {
            library_path: library_path.to_path_buf(),
            tools: vec![],
            sources: vec![],
            sync: crate::core::models::SyncConfig {
                mode: crate::core::models::SyncMode::Symlink,
            },
            install: crate::core::models::InstallConfig {
                allow_zip: true,
                allow_git: true,
                default_sync_targets: vec![],
            },
            exclude_paths: vec![],
            rules: crate::core::models::RulesConfig::default(),
            deleted_skills: vec![],
            debug_logging: false,
        }
    }

    fn create_test_library(root: &Path) -> SkillLibrary {
        let config = create_test_config(&root.join("library"));
        SkillLibrary::new(&config).unwrap()
    }

    fn create_test_database(root: &Path) -> Database {
        Database::new(&root.join("skills.db")).unwrap()
    }

    fn create_skill_dir(path: &Path, skill_name: &str) {
        fs::create_dir_all(path).unwrap();
        fs::write(
            path.join("SKILL.md"),
            format!(
                "---\nname: {}\ndescription: {}\n---\n# body\n",
                skill_name, skill_name
            ),
        )
        .unwrap();
    }

    fn create_skill_zip(zip_path: &Path, skill_root: &str, skill_name: &str) {
        let file = fs::File::create(zip_path).unwrap();
        let mut zip = ZipWriter::new(file);
        let options = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);

        zip.start_file(format!("{}/SKILL.md", skill_root), options)
            .unwrap();
        zip.write_all(
            format!(
                "---\nname: {}\ndescription: zipped skill\n---\n# body\n",
                skill_name
            )
            .as_bytes(),
        )
        .unwrap();
        zip.finish().unwrap();
    }

    #[test]
    fn test_install_local_skill_zip_valid() {
        let temp = TempDir::new().unwrap();
        let library = create_test_library(temp.path());
        let database = create_test_database(temp.path());
        let zip_path = temp.path().join("skill.zip");
        create_skill_zip(&zip_path, "zip-root", "zip-skill");

        let source = crate::core::skill_engine::SkillSource::Zip(zip_path);
        let result = SkillEngine::install_skill(source, &library, &database, None).unwrap();

        assert_eq!(result.library_path, library.skill_path("zip-skill"));
        assert!(library.skill_exists("zip-skill"));
        assert!(library.skill_path("zip-skill").join("SKILL.md").exists());
    }

    #[test]
    fn test_install_local_skill_folder_valid() {
        let temp = TempDir::new().unwrap();
        let library = create_test_library(temp.path());
        let database = create_test_database(temp.path());
        let source = temp.path().join("folder-skill");
        create_skill_dir(&source, "folder-skill");

        let source = crate::core::skill_engine::SkillSource::Folder(source);
        let result = SkillEngine::install_skill(source, &library, &database, None).unwrap();

        assert_eq!(result.library_path, library.skill_path("folder-skill"));
        assert!(library.skill_exists("folder-skill"));
        assert!(library.skill_path("folder-skill").join("SKILL.md").exists());
    }

    #[test]
    fn test_install_local_skill_zip_invalid_rejected() {
        let temp = TempDir::new().unwrap();
        let library = create_test_library(temp.path());
        let database = create_test_database(temp.path());
        let zip_path = temp.path().join("invalid.zip");
        fs::write(&zip_path, b"not a zip archive").unwrap();

        let source = crate::core::skill_engine::SkillSource::Zip(zip_path);
        let result = SkillEngine::install_skill(source, &library, &database, None);

        assert!(matches!(result, Err(AppError::Zip(_))));
    }

    #[test]
    fn test_install_local_skill_folder_legacy_marker() {
        let temp = TempDir::new().unwrap();
        let library = create_test_library(temp.path());
        let database = create_test_database(temp.path());
        let source = temp.path().join("legacy-skill");
        fs::create_dir_all(&source).unwrap();
        fs::write(
            source.join("skill.md"),
            "---\nname: legacy-skill\ndescription: legacy marker\n---\n",
        )
        .unwrap();

        let result = SkillEngine::install_skill(
            crate::core::skill_engine::SkillSource::Folder(source),
            &library,
            &database,
            None,
        )
        .unwrap();

        assert!(library.skill_exists("legacy-skill"));
        assert_eq!(result.library_path, library.skill_path("legacy-skill"));
    }

    #[test]
    fn test_install_local_skill_folder_readme_only_rejected() {
        let temp = TempDir::new().unwrap();
        let library = create_test_library(temp.path());
        let database = create_test_database(temp.path());
        let source = temp.path().join("docs-only");
        fs::create_dir_all(&source).unwrap();
        fs::write(source.join("README.md"), "# just docs").unwrap();
        fs::write(source.join("CLAUDE.md"), "# instructions").unwrap();

        let result = SkillEngine::install_skill(
            crate::core::skill_engine::SkillSource::Folder(source),
            &library,
            &database,
            None,
        );

        assert!(matches!(result, Err(AppError::InvalidSkill(_))));
    }

    #[test]
    fn test_install_local_skill_folder_auto_detects_nested_skill() {
        let temp = TempDir::new().unwrap();
        let library = create_test_library(temp.path());
        let database = create_test_database(temp.path());
        let source = temp.path().join("repo-like");
        fs::create_dir_all(source.join("skills/web-search")).unwrap();
        fs::write(source.join("README.md"), "# repo readme").unwrap();
        fs::write(
            source.join("skills/web-search/SKILL.md"),
            "---\nname: web-search\ndescription: nested\n---\n",
        )
        .unwrap();

        let result = SkillEngine::install_skill(
            crate::core::skill_engine::SkillSource::Folder(source),
            &library,
            &database,
            None,
        )
        .unwrap();

        assert!(library.skill_exists("web-search"));
        assert_eq!(result.library_path, library.skill_path("web-search"));
    }
}
