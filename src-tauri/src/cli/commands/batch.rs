use anyhow::Result;
use clap::{Args, Subcommand};
use std::fs;
use std::path::PathBuf;

use crate::cli::output;
use crate::cli::CliContext;
use crate::core::database::{LinksRepository, SkillsRepository, ToolsRepository};
use crate::core::linker::Linker;

/// Batch operation subcommands
#[derive(Subcommand)]
pub enum BatchCommands {
    /// Batch link skills to tools
    Link(BatchLinkArgs),

    /// Batch delete skills
    Delete(BatchDeleteArgs),

    /// Batch export skills
    Export(BatchExportArgs),
}

/// Arguments for batch link
#[derive(Args)]
pub struct BatchLinkArgs {
    /// Skills to link (comma-separated)
    #[arg(long)]
    pub skills: Option<String>,

    /// Read skills from file (one per line)
    #[arg(long)]
    pub file: Option<PathBuf>,

    /// Tools to link to (comma-separated)
    #[arg(long)]
    pub tools: String,

    /// Pattern to match skill names (e.g., "ai-*")
    #[arg(long)]
    pub pattern: Option<String>,

    /// Link all skills in a group
    #[arg(long)]
    pub group: Option<String>,
}

/// Arguments for batch delete
#[derive(Args)]
pub struct BatchDeleteArgs {
    /// Skills to delete (comma-separated)
    #[arg(long)]
    pub skills: String,

    /// Skip confirmation
    #[arg(long, short)]
    pub force: bool,
}

/// Arguments for batch export
#[derive(Args)]
pub struct BatchExportArgs {
    /// Skills to export (comma-separated)
    #[arg(long)]
    pub skills: Option<String>,

    /// Export only linked skills
    #[arg(long)]
    pub linked: bool,

    /// Output directory
    #[arg(long, short)]
    pub output: PathBuf,
}

/// Execute batch commands
pub fn execute(ctx: CliContext, cmd: BatchCommands) -> Result<()> {
    match cmd {
        BatchCommands::Link(args) => batch_link(ctx, args),
        BatchCommands::Delete(args) => batch_delete(ctx, args),
        BatchCommands::Export(args) => batch_export(ctx, args),
    }
}

/// Batch link skills to tools
fn batch_link(ctx: CliContext, args: BatchLinkArgs) -> Result<()> {
    let skill_names = collect_skill_names(&ctx, &args.skills, &args.file, &args.pattern, &args.group)?;

    if skill_names.is_empty() {
        output::info("No skills to link");
        return Ok(());
    }

    let tool_names: Vec<&str> = args.tools.split(',').collect();
    let skills_repo = SkillsRepository::new(&ctx.database);
    let links_repo = LinksRepository::new(&ctx.database);
    let tools_repo = ToolsRepository::new(&ctx.database);
    let all_tools = tools_repo.get_all()?;

    let mut success_count = 0;
    let mut error_count = 0;

    for skill_name in &skill_names {
        // Get skill from DB
        let skill = match skills_repo.get_by_name(skill_name)? {
            Some(s) => s,
            None => {
                output::error(&format!("Skill '{}' not found", skill_name));
                error_count += 1;
                continue;
            }
        };

        let skill_path = ctx.library.skill_path(skill_name);

        for tool_name in &tool_names {
            let tool_name = tool_name.trim();
            // Find tool by name
            let tool = match all_tools.iter().find(|t| t.name == tool_name) {
                Some(t) => t,
                None => {
                    output::error(&format!("Tool '{}' not found", tool_name));
                    error_count += 1;
                    continue;
                }
            };

            match Linker::link(&skill_path, &tool.expanded_path(), skill_name) {
                Ok(_) => {
                    // Update DB link
                    let _ = links_repo.link(&tool.id, &skill.id);
                    output::verbose(
                        &format!("Linked '{}' to {}", skill_name, tool_name),
                        ctx.verbose,
                    );
                    success_count += 1;
                }
                Err(e) => {
                    output::error(&format!("Failed to link '{}' to {}: {}", skill_name, tool_name, e));
                    error_count += 1;
                }
            }
        }
    }

    output::success(&format!(
        "Batch link complete: {} successful, {} failed",
        success_count, error_count
    ));

    Ok(())
}

/// Batch delete skills
fn batch_delete(ctx: CliContext, args: BatchDeleteArgs) -> Result<()> {
    let skill_names: Vec<&str> = args.skills.split(',').collect();

    if !args.force {
        output::warning(&format!(
            "This will permanently delete {} skills and unlink them from all tools.",
            skill_names.len()
        ));
    }

    let skills_repo = SkillsRepository::new(&ctx.database);

    let mut success_count = 0;
    let mut error_count = 0;

    for skill_name in &skill_names {
        let name = skill_name.trim();

        // Get skill path and remove from disk
        let skill_path = ctx.library.skill_path(name);
        if skill_path.exists() {
            if let Err(e) = fs::remove_dir_all(&skill_path) {
                output::verbose(&format!("Warning removing directory: {}", e), ctx.verbose);
            }
        }

        // Delete from DB
        match skills_repo.delete_by_name(name) {
            Ok(_) => {
                output::verbose(&format!("Deleted '{}'", name), ctx.verbose);
                success_count += 1;
            }
            Err(e) => {
                output::error(&format!("Failed to delete '{}': {}", name, e));
                error_count += 1;
            }
        }
    }

    output::success(&format!(
        "Batch delete complete: {} successful, {} failed",
        success_count, error_count
    ));

    Ok(())
}

/// Batch export skills
fn batch_export(ctx: CliContext, args: BatchExportArgs) -> Result<()> {
    let skill_names: Vec<String> = if args.linked {
        // Get all linked skills
        let skills_repo = SkillsRepository::new(&ctx.database);
        let skills = skills_repo.get_all_active()?;
        skills.into_iter().map(|s| s.name).collect()
    } else if let Some(ref skills) = args.skills {
        skills.split(',').map(|s| s.trim().to_string()).collect()
    } else {
        output::error("Specify --skills or --linked");
        return Ok(());
    };

    // Create output directory
    fs::create_dir_all(&args.output)?;

    let mut success_count = 0;
    for skill_name in skill_names.iter() {
        let skill_path = ctx.library.skill_path(skill_name);
        if skill_path.exists() {
            let dest = args.output.join(skill_name);
            if dest.exists() {
                fs::remove_dir_all(&dest)?;
            }
            copy_dir_all(&skill_path, &dest)?;
            output::verbose(&format!("Exported '{}'", skill_name), ctx.verbose);
            success_count += 1;
        } else {
            output::warning(&format!("Skill '{}' not found on disk", skill_name));
        }
    }

    output::success(&format!("Exported {} skills to {}", success_count, args.output.display()));

    Ok(())
}

/// Collect skill names from various sources
fn collect_skill_names(
    ctx: &CliContext,
    skills: &Option<String>,
    file: &Option<PathBuf>,
    pattern: &Option<String>,
    group: &Option<String>,
) -> Result<Vec<String>> {
    let mut names = Vec::new();

    // From --skills argument
    if let Some(ref s) = skills {
        for name in s.split(',') {
            let trimmed = name.trim().to_string();
            if !trimmed.is_empty() && !names.contains(&trimmed) {
                names.push(trimmed);
            }
        }
    }

    // From --file
    if let Some(ref path) = file {
        let content = fs::read_to_string(path)?;
        for line in content.lines() {
            let trimmed = line.trim().to_string();
            if !trimmed.is_empty() && !trimmed.starts_with('#') && !names.contains(&trimmed) {
                names.push(trimmed);
            }
        }
    }

    // From --pattern or --group (requires DB query)
    if pattern.is_some() || group.is_some() {
        let skills_repo = SkillsRepository::new(&ctx.database);
        let all_skills = skills_repo.get_all_active()?;

        for skill in &all_skills {
            let matches_pattern = pattern
                .as_ref()
                .map(|p| glob_match(p, &skill.name))
                .unwrap_or(true);

            let matches_group = group
                .as_ref()
                .map(|g| skill.group == *g)
                .unwrap_or(true);

            if matches_pattern && matches_group && !names.contains(&skill.name) {
                names.push(skill.name.clone());
            }
        }
    }

    Ok(names)
}

/// Simple glob pattern matching (supports * wildcard)
fn glob_match(pattern: &str, text: &str) -> bool {
    if pattern == "*" {
        return true;
    }

    if let Some(suffix) = pattern.strip_prefix('*') {
        return text.ends_with(suffix);
    }

    if let Some(prefix) = pattern.strip_suffix('*') {
        return text.starts_with(prefix);
    }

    pattern == text
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
