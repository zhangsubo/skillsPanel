pub mod commands;
pub mod output;

use anyhow::Result;
use clap::{Parser, Subcommand};
use std::sync::Arc;

use crate::core::config::AppConfig;
use crate::core::database::Database;
use crate::core::library::SkillLibrary;

/// Skills Panel CLI - Agent Skill Unified Management Tool
#[derive(Parser)]
#[command(name = "skills-cli")]
#[command(version, about, long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Output format
    #[arg(long, global = true, default_value = "table")]
    format: output::OutputFormat,

    /// Verbose output
    #[arg(long, short, global = true)]
    verbose: bool,

    /// Quiet output (only errors)
    #[arg(long, short, global = true)]
    quiet: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// List all skills
    List(commands::list::ListArgs),

    /// Scan and sync skills
    Scan(commands::scan::ScanArgs),

    /// Install a skill (local/zip/git)
    Install(commands::install::InstallArgs),

    /// Uninstall a skill
    Uninstall(commands::uninstall::UninstallArgs),

    /// Link a skill to tools
    Link(commands::link::LinkArgs),

    /// Unlink a skill from tools
    Unlink(commands::unlink::UnlinkArgs),

    /// Batch operations
    #[command(subcommand)]
    Batch(commands::batch::BatchCommands),

    /// Configuration management
    #[command(subcommand)]
    Config(commands::config::ConfigCommands),

    /// Manage tools
    Tools(commands::tools::ToolsArgs),

    /// Export skills
    Export(commands::export::ExportArgs),

    /// Update skills
    Update(commands::update::UpdateArgs),

    /// Tag management
    #[command(subcommand)]
    Tags(commands::tags::TagCommands),
}

/// CLI context holding shared resources
pub struct CliContext {
    pub config: AppConfig,
    pub library: SkillLibrary,
    pub database: Arc<Database>,
    pub output_format: output::OutputFormat,
    pub verbose: bool,
    pub quiet: bool,
}

impl CliContext {
    pub fn new(
        format: output::OutputFormat,
        verbose: bool,
        quiet: bool,
    ) -> Result<Self> {
        let default_config = AppConfig::default_config();
        let database_path = default_config
            .library_path
            .parent()
            .unwrap_or(&default_config.library_path)
            .join("skills_panel.db");

        let database = Database::new(&database_path)?;
        let config = load_config_from_db(&database).unwrap_or(default_config);
        let library = SkillLibrary::new(&config)?;

        Ok(Self {
            config,
            library,
            database: Arc::new(database),
            output_format: format,
            verbose,
            quiet,
        })
    }
}

fn load_config_from_db(database: &Database) -> Result<AppConfig> {
    let config_repo = crate::core::database::ConfigRepository::new(database);
    let mut config = AppConfig::default_config();

    if let Some(path_str) = config_repo.get("library_path")? {
        config.library_path = crate::core::fs_utils::expand_tilde(&path_str);
    }
    if let Some(tools_json) = config_repo.get("tools")? {
        if let Ok(tools) = serde_json::from_str::<Vec<crate::core::models::Tool>>(&tools_json) {
            config.tools = tools;
        }
    }
    if let Some(sources_json) = config_repo.get("sources")? {
        if let Ok(sources) = serde_json::from_str::<Vec<crate::core::models::SourceConfig>>(&sources_json) {
            config.sources = sources;
        }
    }
    if let Some(rules_json) = config_repo.get("rules")? {
        if let Ok(rules) = serde_json::from_str::<crate::core::models::RulesConfig>(&rules_json) {
            config.rules = rules;
        }
    }

    Ok(config)
}

impl Cli {
    pub fn run(self) -> Result<()> {
        let ctx = CliContext::new(self.format, self.verbose, self.quiet)?;

        match self.command {
            Commands::List(args) => commands::list::execute(ctx, args),
            Commands::Scan(args) => commands::scan::execute(ctx, args),
            Commands::Install(args) => commands::install::execute(ctx, args),
            Commands::Uninstall(args) => commands::uninstall::execute(ctx, args),
            Commands::Link(args) => commands::link::execute(ctx, args),
            Commands::Unlink(args) => commands::unlink::execute(ctx, args),
            Commands::Batch(cmd) => commands::batch::execute(ctx, cmd),
            Commands::Config(cmd) => commands::config::execute(ctx, cmd),
            Commands::Tools(args) => commands::tools::execute(ctx, args),
            Commands::Export(args) => commands::export::execute(ctx, args),
            Commands::Update(args) => commands::update::execute(ctx, args),
            Commands::Tags(cmd) => commands::tags::execute(ctx, cmd),
        }
    }
}
