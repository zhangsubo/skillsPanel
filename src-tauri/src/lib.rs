pub mod cli;
pub mod commands;
pub mod core;

use std::sync::Mutex;
use tauri::Emitter;
use tauri::Manager;

use crate::core::audit::AuditLog;
use crate::core::config::AppConfig;
use crate::core::database::Database;
use crate::core::install_cancel::InstallCancelRegistry;
use crate::core::library::SkillLibrary;
use crate::core::models::LogEntry;
use std::sync::Arc;

pub struct AppState {
    pub config: Mutex<AppConfig>,
    pub library: Mutex<SkillLibrary>,
    pub audit_log: Mutex<AuditLog>,
    pub logs: Mutex<Vec<LogEntry>>,
    pub database: Arc<Database>,
    pub cancel_registry: Arc<InstallCancelRegistry>,
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .setup(|app| {
            let default_config = AppConfig::default_config();
            let database_path = default_config
                .library_path
                .parent()
                .unwrap_or(&default_config.library_path)
                .join("skills_panel.db");

            if let Some(parent) = database_path.parent() {
                std::fs::create_dir_all(parent)?;
            }

            let database = Database::new(&database_path)?;

            let mut config = load_config_from_db(&database).unwrap_or_else(|_| {
                let json_config = AppConfig::load_or_create().unwrap_or(default_config);
                migrate_json_config_to_db(&database, &json_config).ok();
                json_config
            });

            let disabled = config.check_tool_availability();
            if !disabled.is_empty() {
                println!(
                    "[Tool Check] Auto-disabled missing tools: {}",
                    disabled.join(", ")
                );
                // Sync updated tools back to DB so subsequent launches see the change
                let tools_json = serde_json::to_string(&config.tools).map_err(|e| {
                    crate::core::error::AppError::Config(format!(
                        "Failed to serialize tools: {}",
                        e
                    ))
                })?;
                let db_repo = crate::core::database::ConfigRepository::new(&database);
                db_repo.set("tools", &tools_json)?;
            }
            let library = SkillLibrary::new(&config)?;
            let audit_log = AuditLog::new(&config)?;

            let migration_result =
                crate::core::migration::Migration::run_on_startup(&database, &config)?;
            if migration_result.config_migrated
                || migration_result.audit_migrated > 0
                || migration_result.tools_migrated > 0
                || migration_result.skills_migrated > 0
            {
                println!("[Migration] {}", migration_result);
            }

            app.manage(Mutex::new(AppState {
                config: Mutex::new(config),
                library: Mutex::new(library),
                audit_log: Mutex::new(audit_log),
                logs: Mutex::new(Vec::new()),
                database: Arc::new(database),
                cancel_registry: Arc::new(InstallCancelRegistry::new()),
            }));

            let config_path = AppConfig::config_path()?;
            let app_handle = app.handle().clone();
            std::thread::spawn(move || {
                use notify::{Config, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
                let Ok(mut watcher) = RecommendedWatcher::new(
                    move |res: Result<notify::Event, notify::Error>| {
                        if let Ok(event) = res {
                            if matches!(event.kind, EventKind::Modify(_)) {
                                let _ = app_handle.emit("config-changed", ());
                            }
                        }
                    },
                    Config::default(),
                ) else {
                    return;
                };
                let _ = watcher.watch(&config_path, RecursiveMode::NonRecursive);
                loop {
                    std::thread::sleep(std::time::Duration::from_secs(1));
                }
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_config,
            commands::update_config,
            commands::get_tools,
            commands::get_library,
            commands::scan_skills,
            commands::get_skill_content,
            commands::preview_local_install,
            commands::preview_git_install,
            commands::install_local_skill,
            commands::install_git_skill,
            commands::link_skill,
            commands::unlink_skill,
            commands::fix_skill_link,
            commands::update_skill_rule,
            commands::update_group_rule,
            commands::update_tool_rule,
            commands::batch_link_skills,
            commands::batch_unlink_skills,
            commands::batch_export_skills,
            commands::batch_delete_skills,
            commands::sync_skills,
            commands::clean_stale_links,
            commands::delete_skill,
            commands::restore_skill,
            commands::export_skill,
            commands::get_audit_logs,
            commands::log_message,
            commands::get_app_logs,
            commands::get_scan_diff,
            commands::get_installed_skills_from_db,
            commands::get_all_active_skills_from_db,
            commands::mark_skill_installed,
            commands::mark_skill_uninstalled,
            commands::upsert_skill_to_db,
            commands::get_config_value,
            commands::set_config_value,
            commands::get_audit_logs_from_db,
            commands::log_audit_entry,
            commands::get_app_logs_from_db,
            commands::log_app_message,
            commands::get_tools_from_db,
            commands::upsert_tool_to_db,
            commands::link_tool_skill_in_db,
            commands::unlink_tool_skill_in_db,
            commands::get_linked_tool_ids,
            commands::cancel_install,
            commands::get_app_version,
            commands::check_skill_update,
            commands::update_skill,
            commands::create_project,
            commands::list_projects,
            commands::delete_project,
            commands::scan_project,
            commands::import_project_skill,
            commands::export_skill_to_project,
            commands::list_tags,
            commands::create_tag,
            commands::update_tag,
            commands::delete_tag,
            commands::attach_tag,
            commands::detach_tag,
            commands::bulk_attach_tag,
            commands::get_skill_tags,
            commands::get_all_skill_tags,
            commands::sync_list_providers,
            commands::sync_add_provider,
            commands::sync_delete_provider,
            commands::sync_test_connection,
            commands::sync_start,
            commands::sync_get_plan,
            commands::sync_get_history,
            commands::sync_rclone_status,
            commands::sync_ensure_rclone,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

fn load_config_from_db(database: &Database) -> Result<AppConfig, crate::core::error::AppError> {
    let db_repo = crate::core::database::ConfigRepository::new(database);
    let mut config = AppConfig::default_config();

    let has_data = db_repo.get("tools")?.is_some()
        || db_repo.get("sources")?.is_some()
        || db_repo.get("library_path")?.is_some();

    if !has_data {
        return Err(crate::core::error::AppError::Config(
            "DB config empty".into(),
        ));
    }

    if let Some(path_str) = db_repo.get("library_path")? {
        config.library_path = crate::core::fs_utils::expand_tilde(&path_str);
    }
    if let Some(tools_json) = db_repo.get("tools")? {
        if let Ok(tools) = serde_json::from_str::<Vec<crate::core::models::Tool>>(&tools_json) {
            config.tools = tools;
        }
    }
    if let Some(sources_json) = db_repo.get("sources")? {
        if let Ok(sources) =
            serde_json::from_str::<Vec<crate::core::models::SourceConfig>>(&sources_json)
        {
            config.sources = sources;
        }
    }
    if let Some(rules_json) = db_repo.get("rules")? {
        if let Ok(rules) = serde_json::from_str::<crate::core::models::RulesConfig>(&rules_json) {
            config.rules = rules;
        }
    }
    if let Some(sync_json) = db_repo.get("sync")? {
        if let Ok(sync) = serde_json::from_str::<crate::core::models::SyncConfig>(&sync_json) {
            config.sync = sync;
        }
    }
    if let Some(install_json) = db_repo.get("install")? {
        if let Ok(install) =
            serde_json::from_str::<crate::core::models::InstallConfig>(&install_json)
        {
            config.install = install;
        }
    }
    if let Some(exclude_json) = db_repo.get("exclude_paths")? {
        if let Ok(exclude) = serde_json::from_str::<Vec<String>>(&exclude_json) {
            config.exclude_paths = exclude;
        }
    }
    if let Some(deleted_json) = db_repo.get("deleted_skills")? {
        if let Ok(deleted) = serde_json::from_str::<Vec<String>>(&deleted_json) {
            config.deleted_skills = deleted;
        }
    }

    Ok(config)
}

fn migrate_json_config_to_db(
    database: &Database,
    config: &AppConfig,
) -> Result<(), crate::core::error::AppError> {
    let db_repo = crate::core::database::ConfigRepository::new(database);

    let tools_json = serde_json::to_string(&config.tools).map_err(|e| {
        crate::core::error::AppError::Config(format!("Failed to serialize tools: {}", e))
    })?;
    let sources_json = serde_json::to_string(&config.sources).map_err(|e| {
        crate::core::error::AppError::Config(format!("Failed to serialize sources: {}", e))
    })?;
    let rules_json = serde_json::to_string(&config.rules).map_err(|e| {
        crate::core::error::AppError::Config(format!("Failed to serialize rules: {}", e))
    })?;
    let sync_json = serde_json::to_string(&config.sync).map_err(|e| {
        crate::core::error::AppError::Config(format!("Failed to serialize sync: {}", e))
    })?;
    let install_json = serde_json::to_string(&config.install).map_err(|e| {
        crate::core::error::AppError::Config(format!("Failed to serialize install: {}", e))
    })?;
    let exclude_json = serde_json::to_string(&config.exclude_paths).map_err(|e| {
        crate::core::error::AppError::Config(format!("Failed to serialize exclude_paths: {}", e))
    })?;
    let deleted_json = serde_json::to_string(&config.deleted_skills).map_err(|e| {
        crate::core::error::AppError::Config(format!("Failed to serialize deleted_skills: {}", e))
    })?;

    db_repo.set("library_path", &config.library_path.to_string_lossy())?;
    db_repo.set("tools", &tools_json)?;
    db_repo.set("sources", &sources_json)?;
    db_repo.set("rules", &rules_json)?;
    db_repo.set("sync", &sync_json)?;
    db_repo.set("install", &install_json)?;
    db_repo.set("exclude_paths", &exclude_json)?;
    db_repo.set("deleted_skills", &deleted_json)?;

    println!("[Migration] Config migrated from JSON to DB");

    Ok(())
}
