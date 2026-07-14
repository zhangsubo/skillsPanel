use anyhow::Result;
use clap::Parser;

use skills_panel::cli::Cli;

fn main() -> Result<()> {
    let cli = Cli::parse();
    cli.run()
}
