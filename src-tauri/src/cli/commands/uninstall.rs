use anyhow::Result;
use clap::Args;

use crate::cli::output;
use crate::cli::CliContext;
use crate::core::database::SkillsRepository;

/// Arguments for the uninstall command
#[derive(Args)]
pub struct UninstallArgs {
    /// Name of the skill to uninstall
    pub name: String,

    /// Skip confirmation prompt
    #[arg(long, short)]
    pub force: bool,
}

/// Execute the uninstall command
pub fn execute(ctx: CliContext, args: UninstallArgs) -> Result<()> {
    // Check if skill exists
    let skills_repo = SkillsRepository::new(&ctx.database);
    let _skill = match skills_repo.get_by_name(&args.name)? {
        Some(s) => s,
        None => {
            output::error(&format!("Skill '{}' not found", args.name));
            return Ok(());
        }
    };

    // Confirm unless force flag is set
    if !args.force {
        output::warning(&format!(
            "This will permanently delete '{}' and unlink it from all tools.",
            args.name
        ));
    }

    // Delete the skill from DB (cascades to links due to FK constraints)
    skills_repo.delete_by_name(&args.name)?;

    // Remove from disk
    let skill_path = ctx.library.skill_path(&args.name);
    if skill_path.exists() {
        std::fs::remove_dir_all(&skill_path)?;
    }

    output::success(&format!("Uninstalled skill: {}", args.name));

    Ok(())
}
