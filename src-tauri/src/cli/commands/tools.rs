use anyhow::Result;
use clap::Args;
use serde::Serialize;
use tabled::Tabled;

use crate::cli::output;
use crate::cli::CliContext;
use crate::core::database::ToolsRepository;

/// Arguments for the tools command
#[derive(Args)]
pub struct ToolsArgs {
    /// Show only enabled tools
    #[arg(long)]
    pub enabled: bool,
}

/// Tool information for display
#[derive(Debug, Serialize, Tabled)]
struct ToolInfo {
    #[tabled(rename = "Name")]
    name: String,

    #[tabled(rename = "Path")]
    path: String,

    #[tabled(rename = "Status")]
    status: String,
}

/// Execute the tools command
pub fn execute(ctx: CliContext, args: ToolsArgs) -> Result<()> {
    let tools_repo = ToolsRepository::new(&ctx.database);

    let mut tools = tools_repo.get_all()?;

    // Filter by enabled status
    if args.enabled {
        tools.retain(|t| t.enabled);
    }

    let tool_infos: Vec<ToolInfo> = tools
        .iter()
        .map(|tool| ToolInfo {
            name: tool.name.clone(),
            path: tool.path.clone(),
            status: if tool.enabled { "Enabled".to_string() } else { "Disabled".to_string() },
        })
        .collect();

    if tool_infos.is_empty() {
        output::info("No tools configured");
    } else {
        output::print_data(&tool_infos, &ctx.output_format);
    }

    Ok(())
}
