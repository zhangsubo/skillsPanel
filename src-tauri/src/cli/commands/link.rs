use anyhow::Result;
use clap::Args;

use crate::cli::output;
use crate::cli::CliContext;
use crate::core::database::{LinksRepository, SkillsRepository, ToolsRepository};
use crate::core::linker::Linker;

/// Arguments for the link command
#[derive(Args)]
pub struct LinkArgs {
    /// Name of the skill to link
    pub skill: String,

    /// Tool to link to (single tool)
    #[arg(long)]
    pub tool: Option<String>,

    /// Multiple tools to link to (comma-separated)
    #[arg(long)]
    pub tools: Option<String>,
}

/// Execute the link command
pub fn execute(ctx: CliContext, args: LinkArgs) -> Result<()> {
    // Collect tool names
    let tool_names = collect_tool_names(&args.tool, &args.tools)?;

    if tool_names.is_empty() {
        output::error("No tools specified. Use --tool or --tools");
        return Ok(());
    }

    // Check if skill exists
    let skills_repo = SkillsRepository::new(&ctx.database);
    let skill = match skills_repo.get_by_name(&args.skill)? {
        Some(s) => s,
        None => {
            output::error(&format!("Skill '{}' not found", args.skill));
            return Ok(());
        }
    };

    let skill_path = ctx.library.skill_path(&args.skill);
    let links_repo = LinksRepository::new(&ctx.database);
    let tools_repo = ToolsRepository::new(&ctx.database);
    let all_tools = tools_repo.get_all()?;

    // Link to each tool
    for tool_name in &tool_names {
        // Find tool by name
        let tool = match all_tools.iter().find(|t| t.name == *tool_name) {
            Some(t) => t,
            None => {
                output::error(&format!("Tool '{}' not found", tool_name));
                continue;
            }
        };

        match Linker::link(&skill_path, &tool.expanded_path(), &args.skill) {
            Ok(_) => {
                // Update DB link
                let _ = links_repo.link(&tool.id, &skill.id);
                output::success(&format!("Linked '{}' to {}", args.skill, tool_name))
            }
            Err(e) => output::error(&format!("Failed to link to {}: {}", tool_name, e)),
        }
    }

    Ok(())
}

/// Collect tool names from --tool and --tools arguments
fn collect_tool_names(tool: &Option<String>, tools: &Option<String>) -> Result<Vec<String>> {
    let mut names = Vec::new();

    if let Some(ref t) = tool {
        names.push(t.clone());
    }

    if let Some(ref ts) = tools {
        for name in ts.split(',') {
            let trimmed = name.trim().to_string();
            if !trimmed.is_empty() && !names.contains(&trimmed) {
                names.push(trimmed);
            }
        }
    }

    Ok(names)
}
