use clap::ValueEnum;
use colored::*;
use serde::Serialize;
use tabled::{Table, Tabled};

/// Output format for CLI commands
#[derive(Debug, Clone, ValueEnum)]
pub enum OutputFormat {
    /// Human-readable table format
    Table,
    /// JSON format for scripting
    Json,
    /// Compact format (one item per line)
    Compact,
}

/// Print data in the specified format
pub fn print_data<T: Serialize + Tabled>(data: &[T], format: &OutputFormat) {
    match format {
        OutputFormat::Table => {
            let table = Table::new(data);
            println!("{table}");
        }
        OutputFormat::Json => {
            let json = serde_json::to_string_pretty(data).unwrap_or_else(|_| "[]".to_string());
            println!("{json}");
        }
        OutputFormat::Compact => {
            for item in data {
                // For compact format, try to print just the name field
                let json = serde_json::to_value(item).unwrap_or_default();
                if let Some(name) = json.get("name").and_then(|n| n.as_str()) {
                    println!("{name}");
                }
            }
        }
    }
}

/// Print a success message
pub fn success(message: &str) {
    println!("{}", format!("✓ {message}").green());
}

/// Print an error message
pub fn error(message: &str) {
    eprintln!("{}", format!("✗ {message}").red());
}

/// Print a warning message
pub fn warning(message: &str) {
    eprintln!("{}", format!("⚠ {message}").yellow());
}

/// Print an info message
pub fn info(message: &str) {
    println!("{}", format!("ℹ {message}").blue());
}

/// Print verbose output (only if verbose mode is enabled)
pub fn verbose(message: &str, is_verbose: bool) {
    if is_verbose {
        println!("{}", format!("  {message}").dimmed());
    }
}

/// Print a simple list of strings
pub fn print_list(items: &[String], format: &OutputFormat) {
    match format {
        OutputFormat::Json => {
            let json = serde_json::to_string_pretty(items).unwrap_or_else(|_| "[]".to_string());
            println!("{json}");
        }
        _ => {
            for item in items {
                println!("{item}");
            }
        }
    }
}

/// Print key-value pairs
pub fn print_config(key: &str, value: &str) {
    println!("{}: {}", key.bold(), value);
}
