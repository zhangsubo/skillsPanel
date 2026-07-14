use anyhow::Result;
use clap::Args;
use std::path::PathBuf;

use crate::cli::output;
use crate::cli::CliContext;
use crate::core::database::{LinksRepository, SkillsRepository, ToolsRepository};
use crate::core::linker::Linker;

/// Arguments for the install command
#[derive(Args)]
pub struct InstallArgs {
    /// Source path (local directory or zip file)
    pub source: String,

    /// Custom name for the skill
    #[arg(long)]
    pub name: Option<String>,

    /// Auto-link to specified tools (comma-separated)
    #[arg(long)]
    pub link: Option<String>,

    /// Force overwrite if exists
    #[arg(long, short)]
    pub force: bool,
}

/// Execute the install command
pub fn execute(ctx: CliContext, args: InstallArgs) -> Result<()> {
    output::info(&format!("Installing skill from: {}", args.source));

    let source_path = PathBuf::from(&args.source);
    if !source_path.exists() {
        output::error(&format!("Source path does not exist: {}", args.source));
        return Ok(());
    }

    // Determine skill name
    let skill_name = args.name.clone().unwrap_or_else(|| {
        source_path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "unnamed-skill".to_string())
    });

    // Add skill to library
    let skill_path = if args.force {
        ctx.library.add_skill_with_overwrite(&source_path, &skill_name)?
    } else {
        match ctx.library.add_skill(&source_path, &skill_name) {
            Ok(path) => path,
            Err(e) => {
                output::error(&format!("Installation failed: {}", e));
                return Ok(());
            }
        }
    };

    // Register in database
    let skills_repo = SkillsRepository::new(&ctx.database);
    let skill_id = crate::core::library::SkillLibrary::compute_skill_id(&skill_name, &skill_path);
    let skill = crate::core::models::Skill {
        id: skill_id.clone(),
        name: skill_name.clone(),
        path_hash: crate::core::library::SkillLibrary::compute_path_hash(&skill_path),
        library_path: skill_path.to_string_lossy().to_string(),
        original_source_path: Some(source_path.to_string_lossy().to_string()),
        original_git_url: None,
        original_git_subpath: None,
        group: "default".to_string(),
        description: String::new(),
        frontmatter: std::collections::HashMap::new(),
        created_at: chrono::Utc::now().to_rfc3339(),
        mtime_ms: 0,
        source_type: crate::core::models::SkillSourceType::LocalFolder,
        is_deleted: false,
        content_hash: None,
        source_revision: None,
        source_remote_revision: None,
        source_update_status: crate::core::models::SourceUpdateStatus::Unknown,
    };

    skills_repo.upsert(&skill)?;
    output::success(&format!("Installed skill: {}", skill_name));

    // Auto-link if requested
    if let Some(ref tools) = args.link {
        let tool_list: Vec<&str> = tools.split(',').collect();
        let links_repo = LinksRepository::new(&ctx.database);
        let tools_repo = ToolsRepository::new(&ctx.database);
        let all_tools = tools_repo.get_all()?;

        for tool_name in tool_list {
            let tool_name = tool_name.trim();
            if let Some(tool) = all_tools.iter().find(|t| t.name == tool_name) {
                match Linker::link(&skill_path, &tool.expanded_path(), &skill_name) {
                    Ok(_) => {
                        let _ = links_repo.link(&tool.id, &skill_id);
                        output::success(&format!("Linked to {}", tool_name))
                    }
                    Err(e) => output::warning(&format!("Failed to link to {}: {}", tool_name, e)),
                }
            } else {
                output::warning(&format!("Tool '{}' not found", tool_name));
            }
        }
    }

    Ok(())
}
