use anyhow::Result;
use clap::Args;
use std::fs;
use std::path::PathBuf;

use crate::cli::output;
use crate::cli::CliContext;
use crate::core::database::SkillsRepository;

/// Arguments for the export command
#[derive(Args)]
pub struct ExportArgs {
    /// Name of the skill to export
    pub skill: String,

    /// Output directory
    #[arg(long, short)]
    pub output: PathBuf,
}

/// Execute the export command
pub fn execute(ctx: CliContext, args: ExportArgs) -> Result<()> {
    // Check if skill exists
    let skills_repo = SkillsRepository::new(&ctx.database);
    if skills_repo.get_by_name(&args.skill)?.is_none() {
        output::error(&format!("Skill '{}' not found", args.skill));
        return Ok(());
    }

    let skill_path = ctx.library.skill_path(&args.skill);
    if !skill_path.exists() {
        output::error(&format!("Skill directory not found: {}", skill_path.display()));
        return Ok(());
    }

    // Create output directory
    fs::create_dir_all(&args.output)?;

    // Copy skill to output
    let dest = args.output.join(&args.skill);
    if dest.exists() {
        fs::remove_dir_all(&dest)?;
    }
    copy_dir_all(&skill_path, &dest)?;

    output::success(&format!(
        "Exported '{}' to {}",
        args.skill,
        dest.display()
    ));

    Ok(())
}

/// Recursively copy a directory
fn copy_dir_all(src: &PathBuf, dst: &PathBuf) -> Result<()> {
    fs::create_dir_all(dst)?;

    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        let dest = dst.join(entry.file_name());

        if ty.is_dir() {
            copy_dir_all(&entry.path(), &dest)?;
        } else {
            fs::copy(entry.path(), dest)?;
        }
    }

    Ok(())
}
