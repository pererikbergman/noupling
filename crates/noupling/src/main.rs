mod cli;
mod core;
mod slices;
mod utils;

use clap::Parser;
use cli::{Cli, Commands};
use std::path::Path;

fn main() {
    let cli = Cli::parse();

    let result = match cli.command {
        Commands::Scan { path } => run_scan(&path),
        Commands::Audit { snapshot } => run_audit(snapshot.as_deref()),
        Commands::Report { format } => run_report(&format),
    };

    if let Err(e) = result {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}

fn find_db() -> anyhow::Result<slices::storage::Database> {
    let db_path = Path::new(".noupling").join("history.db");
    if !db_path.exists() {
        anyhow::bail!("No database found. Run `noupling scan <PATH>` first.");
    }
    slices::storage::Database::open(&db_path)
}

fn run_scan(path: &str) -> anyhow::Result<()> {
    let project_path = Path::new(path);
    if !project_path.exists() {
        anyhow::bail!("Path does not exist: {}", path);
    }

    println!("Scanning: {}", path);

    let db_path = project_path.join(".noupling").join("history.db");
    let db = slices::storage::Database::open(&db_path)?;

    let snap_repo = slices::storage::repository::SnapshotRepository::new(&db.conn);
    let snapshot = snap_repo.create(path)?;
    println!("Created snapshot: {}", snapshot.id);

    let result = slices::scanner::scan_project(project_path, &snapshot.id)?;
    println!("Discovered {} modules", result.modules.len());

    let module_repo = slices::storage::repository::ModuleRepository::new(&db.conn);
    module_repo.bulk_insert(&result.modules)?;

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

fn run_audit(snapshot_id: Option<&str>) -> anyhow::Result<()> {
    let db = find_db()?;
    let snap_repo = slices::storage::repository::SnapshotRepository::new(&db.conn);

    let snapshot = match snapshot_id {
        Some(id) => snap_repo
            .get_by_id(id)?
            .ok_or_else(|| anyhow::anyhow!("Snapshot not found: {}", id))?,
        None => snap_repo
            .get_latest()?
            .ok_or_else(|| anyhow::anyhow!("No snapshots found. Run `noupling scan` first."))?,
    };

    let module_repo = slices::storage::repository::ModuleRepository::new(&db.conn);
    let dep_repo = slices::storage::repository::DependencyRepository::new(&db.conn);

    let modules = module_repo.get_by_snapshot(&snapshot.id)?;
    let dependencies = dep_repo.get_by_snapshot(&snapshot.id)?;

    let result = slices::analyzer::audit(&modules, &dependencies);

    print!("{}", slices::reporter::format_text(&result));

    Ok(())
}

fn run_report(format: &str) -> anyhow::Result<()> {
    let db = find_db()?;
    let snap_repo = slices::storage::repository::SnapshotRepository::new(&db.conn);

    let snapshot = snap_repo
        .get_latest()?
        .ok_or_else(|| anyhow::anyhow!("No snapshots found. Run `noupling scan` first."))?;

    let module_repo = slices::storage::repository::ModuleRepository::new(&db.conn);
    let dep_repo = slices::storage::repository::DependencyRepository::new(&db.conn);

    let modules = module_repo.get_by_snapshot(&snapshot.id)?;
    let dependencies = dep_repo.get_by_snapshot(&snapshot.id)?;

    let result = slices::analyzer::audit(&modules, &dependencies);

    match format {
        "json" => {
            let report = slices::reporter::JsonReport::from_audit(&result, &snapshot.id);
            println!("{}", report.to_json()?);
        }
        "md" => {
            println!("{}", slices::reporter::format_markdown(&result, &snapshot.id));
        }
        _ => {
            anyhow::bail!("Unknown format: {}. Use 'json' or 'md'.", format);
        }
    }

    Ok(())
}
