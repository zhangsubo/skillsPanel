use anyhow::Result;
use clap::Args;

use crate::cli::output;
use crate::cli::CliContext;
use crate::core::models::SkillToolStatus;
use crate::core::scanner::Scanner;

/// Arguments for the scan command
#[derive(Args)]
pub struct ScanArgs {
    /// Show diff after scan
    #[arg(long)]
    pub diff: bool,

    /// Show only conflicts
    #[arg(long)]
    pub conflicts_only: bool,
}

/// Execute the scan command
pub fn execute(ctx: CliContext, args: ScanArgs) -> Result<()> {
    output::info("Scanning skills...");

    // Use the scanner module to scan all sources
    let results = Scanner::scan_sources(&ctx.config, &ctx.library)?;

    if args.conflicts_only {
        let conflicts: Vec<_> = results
            .iter()
            .filter(|r| {
                r.tool_statuses
                    .values()
                    .any(|s| matches!(s, SkillToolStatus::Wrong))
            })
            .collect();

        if conflicts.is_empty() {
            output::success("No conflicts found");
        } else {
            output::warning(&format!("Found {} conflicts:", conflicts.len()));
            for conflict in &conflicts {
                output::error(&format!("  {}", conflict.skill.name));
            }
        }
    } else {
        output::success(&format!("Scan complete. Found {} skills", results.len()));

        if args.diff {
            // Show differences
            for result in &results {
                let status_summary = if result.tool_statuses.is_empty() {
                    "unlinked"
                } else if result
                    .tool_statuses
                    .values()
                    .any(|s| matches!(s, SkillToolStatus::Linked))
                {
                    "linked"
                } else {
                    "unlinked"
                };
                output::info(&format!("  {} ({})", result.skill.name, status_summary));
            }
        }
    }

    Ok(())
}
