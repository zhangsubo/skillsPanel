use anyhow::Result;
use clap::Args;

use crate::cli::output;
use crate::cli::CliContext;
use crate::core::database::{LinksRepository, SkillsRepository, ToolsRepository};
use crate::core::linker::Linker;

/// Arguments for the unlink command
#[derive(Args)]
pub struct UnlinkArgs {
    /// Name of the skill to unlink
    pub skill: String,

    /// Tool to unlink from (single tool)
    #[arg(long)]
    pub tool: Option<String>,

    /// Unlink from all tools
    #[arg(long)]
    pub all: bool,
}

/// Execute the unlink command
pub fn execute(ctx: CliContext, args: UnlinkArgs) -> Result<()> {
    // Check if skill exists
    let skills_repo = SkillsRepository::new(&ctx.database);
    let skill = match skills_repo.get_by_name(&args.skill)? {
        Some(s) => s,
        None => {
            output::error(&format!("Skill '{}' not found", args.skill));
            return Ok(());
        }
    };

    let links_repo = LinksRepository::new(&ctx.database);
    let tools_repo = ToolsRepository::new(&ctx.database);
    let all_tools = tools_repo.get_all()?;

    if args.all {
        // Unlink from all tools
        let linked_tool_ids = links_repo.get_linked_tool_ids(&skill.id)?;

        for tool_id in &linked_tool_ids {
            if let Some(tool) = all_tools.iter().find(|t| t.id == *tool_id) {
                let _ = Linker::unlink(&tool.expanded_path(), &args.skill);
                let _ = links_repo.unlink(tool_id, &skill.id);
            }
        }

        output::success(&format!("Unlinked '{}' from all tools", args.skill));
    } else if let Some(ref tool_name) = args.tool {
        // Unlink from specific tool
        let tool = match all_tools.iter().find(|t| t.name == *tool_name) {
            Some(t) => t,
            None => {
                output::error(&format!("Tool '{}' not found", tool_name));
                return Ok(());
            }
        };

        match Linker::unlink(&tool.expanded_path(), &args.skill) {
            Ok(_) => {
                let _ = links_repo.unlink(&tool.id, &skill.id);
                output::success(&format!("Unlinked '{}' from {}", args.skill, tool_name))
            }
            Err(e) => output::error(&format!("Failed to unlink from {}: {}", tool_name, e)),
        }
    } else {
        output::error("Specify --tool or --all");
    }

    Ok(())
}
