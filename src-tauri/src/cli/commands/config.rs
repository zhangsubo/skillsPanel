use anyhow::Result;
use clap::{Args, Subcommand};

use crate::cli::output;
use crate::cli::CliContext;
use crate::core::database::{ConfigRepository, ToolsRepository};

/// Configuration subcommands
#[derive(Subcommand)]
pub enum ConfigCommands {
    /// Show current configuration
    Show,

    /// Set a configuration value
    Set(ConfigSetArgs),

    /// Add a new tool
    AddTool(ConfigAddToolArgs),

    /// Add a new source
    AddSource(ConfigAddSourceArgs),

    /// List all configured tools
    ListTools,

    /// Enable a tool
    Enable(ConfigEnableArgs),

    /// Disable a tool
    Disable(ConfigDisableArgs),
}

/// Arguments for config set
#[derive(Args)]
pub struct ConfigSetArgs {
    /// Configuration key
    pub key: String,

    /// Configuration value
    pub value: String,
}

/// Arguments for config add-tool
#[derive(Args)]
pub struct ConfigAddToolArgs {
    /// Tool name
    #[arg(long)]
    pub name: String,

    /// Tool directory path
    #[arg(long)]
    pub path: String,
}

/// Arguments for config add-source
#[derive(Args)]
pub struct ConfigAddSourceArgs {
    /// Source path
    #[arg(long)]
    pub path: String,

    /// Group name
    #[arg(long)]
    pub group: Option<String>,
}

/// Arguments for config enable/disable
#[derive(Args)]
pub struct ConfigEnableArgs {
    /// Tool name
    pub tool: String,
}

/// Arguments for config disable
#[derive(Args)]
pub struct ConfigDisableArgs {
    /// Tool name
    pub tool: String,
}

/// Execute config commands
pub fn execute(ctx: CliContext, cmd: ConfigCommands) -> Result<()> {
    match cmd {
        ConfigCommands::Show => config_show(ctx),
        ConfigCommands::Set(args) => config_set(ctx, args),
        ConfigCommands::AddTool(args) => config_add_tool(ctx, args),
        ConfigCommands::AddSource(args) => config_add_source(ctx, args),
        ConfigCommands::ListTools => config_list_tools(ctx),
        ConfigCommands::Enable(args) => config_enable(ctx, args),
        ConfigCommands::Disable(args) => config_disable(ctx, args),
    }
}

/// Show current configuration
fn config_show(ctx: CliContext) -> Result<()> {
    let library_path = ctx.config.library_path.to_string_lossy().to_string();
    output::print_config("Library Path", &library_path);

    output::print_config("Tools", &format!("{} configured", ctx.config.tools.len()));

    for tool in &ctx.config.tools {
        let status = if tool.enabled { "✓" } else { "✗" };
        println!("  {} {} - {}", status, tool.name, tool.path);
    }

    Ok(())
}

/// Set a configuration value
fn config_set(ctx: CliContext, args: ConfigSetArgs) -> Result<()> {
    let config_repo = ConfigRepository::new(&ctx.database);

    match args.key.as_str() {
        "library_path" => {
            config_repo.set("library_path", &args.value)?;
            output::success("Library path updated");
        }
        _ => {
            output::error(&format!("Unknown configuration key: {}", args.key));
            output::info("Available keys: library_path");
        }
    }

    Ok(())
}

/// Add a new tool
fn config_add_tool(ctx: CliContext, args: ConfigAddToolArgs) -> Result<()> {
    let tools_repo = ToolsRepository::new(&ctx.database);

    // Check if tool already exists
    let all_tools = tools_repo.get_all()?;
    if all_tools.iter().any(|t| t.name == args.name) {
        output::error(&format!("Tool '{}' already exists", args.name));
        return Ok(());
    }

    // Create new tool
    let tool = crate::core::models::Tool {
        id: uuid::Uuid::new_v4().to_string(),
        name: args.name.clone(),
        path: args.path.clone(),
        enabled: true,
        is_custom: true,
    };

    tools_repo.upsert(&tool)?;
    output::success(&format!("Added tool: {}", args.name));

    Ok(())
}

/// Add a new source
fn config_add_source(ctx: CliContext, args: ConfigAddSourceArgs) -> Result<()> {
    let config_repo = ConfigRepository::new(&ctx.database);

    // Get current sources
    let mut sources: Vec<String> = config_repo
        .get("sources")
        .unwrap_or(None)
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();

    // Add new source
    let source_entry = if let Some(ref group) = args.group {
        format!("{}:{}", args.path, group)
    } else {
        args.path.clone()
    };

    if sources.contains(&source_entry) {
        output::warning("Source already exists");
        return Ok(());
    }

    sources.push(source_entry);
    config_repo.set("sources", &serde_json::to_string(&sources)?)?;

    output::success("Source added");

    Ok(())
}

/// List all configured tools
fn config_list_tools(ctx: CliContext) -> Result<()> {
    let tools_repo = ToolsRepository::new(&ctx.database);
    let tools = tools_repo.get_all()?;

    if tools.is_empty() {
        output::info("No tools configured");
    } else {
        for tool in &tools {
            let status = if tool.enabled { "✓" } else { "✗" };
            println!("{} {} - {}", status, tool.name, tool.path);
        }
    }

    Ok(())
}

/// Enable a tool
fn config_enable(ctx: CliContext, args: ConfigEnableArgs) -> Result<()> {
    let tools_repo = ToolsRepository::new(&ctx.database);
    let tools = tools_repo.get_all()?;

    match tools.into_iter().find(|t| t.name == args.tool) {
        Some(mut tool) => {
            tool.enabled = true;
            tools_repo.upsert(&tool)?;
            output::success(&format!("Enabled tool: {}", args.tool));
        }
        None => {
            output::error(&format!("Tool '{}' not found", args.tool));
        }
    }

    Ok(())
}

/// Disable a tool
fn config_disable(ctx: CliContext, args: ConfigDisableArgs) -> Result<()> {
    let tools_repo = ToolsRepository::new(&ctx.database);
    let tools = tools_repo.get_all()?;

    match tools.into_iter().find(|t| t.name == args.tool) {
        Some(mut tool) => {
            tool.enabled = false;
            tools_repo.upsert(&tool)?;
            output::success(&format!("Disabled tool: {}", args.tool));
        }
        None => {
            output::error(&format!("Tool '{}' not found", args.tool));
        }
    }

    Ok(())
}
