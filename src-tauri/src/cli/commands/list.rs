use anyhow::Result;
use clap::Args;
use serde::Serialize;
use tabled::Tabled;

use crate::cli::output;
use crate::cli::CliContext;
use crate::core::database::{LinksRepository, SkillsRepository, ToolsRepository};

/// Arguments for the list command
#[derive(Args)]
pub struct ListArgs {
    /// Filter by tool name
    #[arg(long)]
    pub tool: Option<String>,

    /// Show only linked skills
    #[arg(long)]
    pub linked: bool,

    /// Filter by group
    #[arg(long)]
    pub group: Option<String>,
}

/// Skill information for display
#[derive(Debug, Serialize, Tabled)]
struct SkillInfo {
    #[tabled(rename = "Name")]
    name: String,

    #[tabled(rename = "Group")]
    group: String,

    #[tabled(rename = "Source")]
    source: String,

    #[tabled(rename = "Tools")]
    tools: String,

    #[tabled(rename = "Status")]
    status: String,
}

/// Execute the list command
pub fn execute(ctx: CliContext, args: ListArgs) -> Result<()> {
    let skills_repo = SkillsRepository::new(&ctx.database);
    let links_repo = LinksRepository::new(&ctx.database);
    let tools_repo = ToolsRepository::new(&ctx.database);

    // Get all skills
    let mut skills = skills_repo.get_all_active()?;

    // Apply group filter
    if let Some(ref group) = args.group {
        skills.retain(|s| s.group == *group);
    }

    // Get all tools for mapping tool_id to name
    let all_tools = tools_repo.get_all()?;
    let tool_map: std::collections::HashMap<String, String> = all_tools
        .into_iter()
        .map(|t| (t.id, t.name))
        .collect();

    // Get link information for each skill
    let skill_infos: Vec<SkillInfo> = skills
        .iter()
        .filter_map(|skill| {
            let tool_ids = links_repo.get_linked_tool_ids(&skill.id).unwrap_or_default();
            let tool_names: Vec<String> = tool_ids
                .iter()
                .filter_map(|id| tool_map.get(id).cloned())
                .collect();

            // Filter by tool if specified
            if let Some(ref tool) = args.tool {
                if !tool_names.contains(tool) {
                    return None;
                }
            }

            // Filter by linked status
            if args.linked && tool_names.is_empty() {
                return None;
            }

            let status = if tool_names.is_empty() {
                "Unlinked".to_string()
            } else {
                "Linked".to_string()
            };

            let source_str = match skill.source_type {
                crate::core::models::SkillSourceType::LocalFolder => "local-folder",
                crate::core::models::SkillSourceType::LocalZip => "local-zip",
                crate::core::models::SkillSourceType::Git => "git",
            };

            Some(SkillInfo {
                name: skill.name.clone(),
                group: skill.group.clone(),
                source: source_str.to_string(),
                tools: if tool_names.is_empty() {
                    "-".to_string()
                } else {
                    tool_names.join(", ")
                },
                status,
            })
        })
        .collect();

    // Output results
    if skill_infos.is_empty() {
        output::info("No skills found matching the criteria");
    } else {
        output::print_data(&skill_infos, &ctx.output_format);
    }

    Ok(())
}
