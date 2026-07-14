use anyhow::Result;
use clap::{Args, Subcommand};
use serde::Serialize;
use tabled::Tabled;

use crate::cli::output;
use crate::cli::CliContext;
use crate::core::database::{SkillsRepository, TagsRepository};

/// Tag subcommands
#[derive(Subcommand)]
pub enum TagCommands {
    /// List all tags
    List,

    /// Create a new tag
    Create(TagCreateArgs),

    /// Attach a tag to a skill
    Attach(TagAttachArgs),

    /// Detach a tag from a skill
    Detach(TagDetachArgs),

    /// Batch attach tag to multiple skills
    AttachBatch(TagAttachBatchArgs),
}

/// Arguments for tag create
#[derive(Args)]
pub struct TagCreateArgs {
    /// Tag name
    #[arg(long)]
    pub name: String,

    /// Tag color (hex)
    #[arg(long)]
    pub color: Option<String>,
}

/// Arguments for tag attach
#[derive(Args)]
pub struct TagAttachArgs {
    /// Skill name
    #[arg(long)]
    pub skill: String,

    /// Tag name
    #[arg(long)]
    pub tag: String,
}

/// Arguments for tag detach
#[derive(Args)]
pub struct TagDetachArgs {
    /// Skill name
    #[arg(long)]
    pub skill: String,

    /// Tag name
    #[arg(long)]
    pub tag: String,
}

/// Arguments for tag attach-batch
#[derive(Args)]
pub struct TagAttachBatchArgs {
    /// Skills to tag (comma-separated)
    #[arg(long)]
    pub skills: String,

    /// Tag name
    #[arg(long)]
    pub tag: String,
}

/// Tag information for display
#[derive(Debug, Serialize, Tabled)]
struct TagInfo {
    #[tabled(rename = "ID")]
    id: String,

    #[tabled(rename = "Name")]
    name: String,

    #[tabled(rename = "Color")]
    color: String,
}

/// Execute tag commands
pub fn execute(ctx: CliContext, cmd: TagCommands) -> Result<()> {
    match cmd {
        TagCommands::List => tag_list(ctx),
        TagCommands::Create(args) => tag_create(ctx, args),
        TagCommands::Attach(args) => tag_attach(ctx, args),
        TagCommands::Detach(args) => tag_detach(ctx, args),
        TagCommands::AttachBatch(args) => tag_attach_batch(ctx, args),
    }
}

/// List all tags
fn tag_list(ctx: CliContext) -> Result<()> {
    let tags_repo = TagsRepository::new(&ctx.database);
    let tags = tags_repo.list()?;

    if tags.is_empty() {
        output::info("No tags found");
    } else {
        let tag_infos: Vec<TagInfo> = tags
            .iter()
            .map(|tag| TagInfo {
                id: tag.id.clone(),
                name: tag.name.clone(),
                color: tag.color.clone().unwrap_or_else(|| "#888888".to_string()),
            })
            .collect();
        output::print_data(&tag_infos, &ctx.output_format);
    }

    Ok(())
}

/// Create a new tag
fn tag_create(ctx: CliContext, args: TagCreateArgs) -> Result<()> {
    let tags_repo = TagsRepository::new(&ctx.database);

    let tag = tags_repo.create(
        &args.name,
        args.color.as_deref(),
        None, // description
    )?;
    output::success(&format!("Tag '{}' created", tag.name));
    Ok(())
}

/// Attach a tag to a skill
fn tag_attach(ctx: CliContext, args: TagAttachArgs) -> Result<()> {
    let skills_repo = SkillsRepository::new(&ctx.database);
    let tags_repo = TagsRepository::new(&ctx.database);

    // Find skill
    let skill = match skills_repo.get_by_name(&args.skill)? {
        Some(s) => s,
        None => {
            output::error(&format!("Skill '{}' not found", args.skill));
            return Ok(());
        }
    };

    // Find tag by name
    let tag = match tags_repo.get_by_name(&args.tag)? {
        Some(t) => t,
        None => {
            output::error(&format!("Tag '{}' not found", args.tag));
            return Ok(());
        }
    };

    tags_repo.attach(&skill.id, &tag.id)?;
    output::success(&format!("Attached tag '{}' to '{}'", args.tag, args.skill));
    Ok(())
}

/// Detach a tag from a skill
fn tag_detach(ctx: CliContext, args: TagDetachArgs) -> Result<()> {
    let skills_repo = SkillsRepository::new(&ctx.database);
    let tags_repo = TagsRepository::new(&ctx.database);

    // Find skill
    let skill = match skills_repo.get_by_name(&args.skill)? {
        Some(s) => s,
        None => {
            output::error(&format!("Skill '{}' not found", args.skill));
            return Ok(());
        }
    };

    // Find tag by name
    let tag = match tags_repo.get_by_name(&args.tag)? {
        Some(t) => t,
        None => {
            output::error(&format!("Tag '{}' not found", args.tag));
            return Ok(());
        }
    };

    tags_repo.detach(&skill.id, &tag.id)?;
    output::success(&format!("Detached tag '{}' from '{}'", args.tag, args.skill));
    Ok(())
}

/// Batch attach tag to multiple skills
fn tag_attach_batch(ctx: CliContext, args: TagAttachBatchArgs) -> Result<()> {
    let skill_names: Vec<&str> = args.skills.split(',').collect();
    let skills_repo = SkillsRepository::new(&ctx.database);
    let tags_repo = TagsRepository::new(&ctx.database);

    // Find tag by name
    let tag = match tags_repo.get_by_name(&args.tag)? {
        Some(t) => t,
        None => {
            output::error(&format!("Tag '{}' not found", args.tag));
            return Ok(());
        }
    };

    let mut success_count = 0;
    let mut error_count = 0;

    for skill_name in &skill_names {
        let name = skill_name.trim();

        match skills_repo.get_by_name(name)? {
            Some(skill) => {
                match tags_repo.attach(&skill.id, &tag.id) {
                    Ok(_) => success_count += 1,
                    Err(e) => {
                        output::error(&format!("Failed to tag '{}': {}", name, e));
                        error_count += 1;
                    }
                }
            }
            None => {
                output::error(&format!("Skill '{}' not found", name));
                error_count += 1;
            }
        }
    }

    output::success(&format!(
        "Batch tag attach complete: {} successful, {} failed",
        success_count, error_count
    ));

    Ok(())
}
