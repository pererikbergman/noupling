mod analyzer;
mod baseline;
mod cli;
mod commands;
mod core;
mod diff;
mod hook;
mod reporter;
mod scanner;
pub mod settings;
mod storage;
mod utils;

use clap::Parser;
use cli::{Cli, Commands};
use std::path::Path;

fn main() {
    let cli = Cli::parse();

    // Ensure settings.json exists for any command that takes a path
    match &cli.command {
        Commands::Init { path }
        | Commands::Scan { path, .. }
        | Commands::Hook { path, .. }
        | Commands::Baseline { path, .. }
        | Commands::Audit { path, .. }
        | Commands::Trend { path, .. }
        | Commands::Report { path, .. } => {
            let settings_path = Path::new(path).join(".noupling").join("settings.json");
            if !settings_path.exists() {
                let _ = settings::Settings::write_defaults(Path::new(path));
            }
        }
    }

    let result = match cli.command {
        Commands::Init { path } => commands::init::run(&path),
        Commands::Hook { action, path } => commands::hook::run(&action, &path),
        Commands::Scan { path, diff_base } => commands::scan::run(&path, diff_base.as_deref()),
        Commands::Baseline { action, path } => commands::baseline::run(&action, &path),
        Commands::Audit {
            path,
            snapshot,
            fail_below,
            baseline,
            module,
        } => commands::audit::run(
            &path,
            snapshot.as_deref(),
            fail_below,
            baseline,
            module.as_deref(),
        ),
        Commands::Trend {
            path,
            last,
            by_module,
        } => commands::trend::run(&path, last, by_module),
        Commands::Report {
            path,
            format,
            module,
            last,
        } => commands::report::run(&path, &format, module.as_deref(), last),
    };

    if let Err(e) = result {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}
