mod cli;
mod core;
mod slices;
mod utils;

use clap::Parser;
use cli::{Cli, Commands};
use std::path::Path;

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Scan { path } => {
            if let Err(e) = run_scan(&path) {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        }
        Commands::Audit { snapshot } => {
            match snapshot {
                Some(id) => println!("Auditing snapshot: {}", id),
                None => println!("Auditing latest snapshot..."),
            }
            println!("Not yet implemented.");
        }
        Commands::Report { format } => {
            println!("Generating {} report...", format);
            println!("Not yet implemented.");
        }
    }
}

fn run_scan(path: &str) -> anyhow::Result<()> {
    let project_path = Path::new(path);
    if !project_path.exists() {
        anyhow::bail!("Path does not exist: {}", path);
    }

    println!("Scanning: {}", path);

    // Open database
    let db_path = project_path.join(".noupling").join("history.db");
    let db = slices::storage::Database::open(&db_path)?;

    // Create snapshot
    let snap_repo = slices::storage::repository::SnapshotRepository::new(&db.conn);
    let snapshot = snap_repo.create(path)?;
    println!("Created snapshot: {}", snapshot.id);

    // Scan project
    let result = slices::scanner::scan_project(project_path, &snapshot.id)?;
    println!("Discovered {} modules", result.modules.len());

    // Store modules
    let module_repo = slices::storage::repository::ModuleRepository::new(&db.conn);
    module_repo.bulk_insert(&result.modules)?;

    // Deduplicate and store dependencies
    let mut unique_deps = result.dependencies;
    unique_deps.sort_by(|a, b| {
        (&a.from_module_id, &a.to_module_id, &a.line_number)
            .cmp(&(&b.from_module_id, &b.to_module_id, &b.line_number))
    });
    unique_deps.dedup_by(|a, b| {
        a.from_module_id == b.from_module_id
            && a.to_module_id == b.to_module_id
            && a.line_number == b.line_number
    });

    let dep_repo = slices::storage::repository::DependencyRepository::new(&db.conn);
    dep_repo.bulk_insert(&unique_deps)?;
    println!("Found {} dependencies", unique_deps.len());

    println!("Scan complete. Database: {}", db_path.display());
    Ok(())
}
