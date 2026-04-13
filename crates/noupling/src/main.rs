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
        Commands::Audit { path, snapshot } => run_audit(&path, snapshot.as_deref()),
        Commands::Report { path, format } => run_report(&path, &format),
    };

    if let Err(e) = result {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}

fn find_db(project_path: &str) -> anyhow::Result<slices::storage::Database> {
    let db_path = Path::new(project_path).join(".noupling").join("history.db");
    if !db_path.exists() {
        anyhow::bail!("No database found at {}. Run `noupling scan <PATH>` first.", db_path.display());
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

fn run_audit(path: &str, snapshot_id: Option<&str>) -> anyhow::Result<()> {
    let db = find_db(path)?;
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

fn run_report(path: &str, format: &str) -> anyhow::Result<()> {
    let db = find_db(path)?;
    let snap_repo = slices::storage::repository::SnapshotRepository::new(&db.conn);

    let snapshot = snap_repo
        .get_latest()?
        .ok_or_else(|| anyhow::anyhow!("No snapshots found. Run `noupling scan` first."))?;

    let module_repo = slices::storage::repository::ModuleRepository::new(&db.conn);
    let dep_repo = slices::storage::repository::DependencyRepository::new(&db.conn);

    let modules = module_repo.get_by_snapshot(&snapshot.id)?;
    let dependencies = dep_repo.get_by_snapshot(&snapshot.id)?;

    let result = slices::analyzer::audit(&modules, &dependencies);

    let report_dir = Path::new(path).join(".noupling");
    std::fs::create_dir_all(&report_dir)?;

    match format {
        "json" => {
            let report = slices::reporter::JsonReport::from_audit(&result, &snapshot.id);
            let content = report.to_json()?;
            let file_path = report_dir.join("report.json");
            std::fs::write(&file_path, &content)?;
            println!("{}", content);
            println!("\nReport saved to {}", file_path.display());
        }
        "md" => {
            let content = slices::reporter::format_markdown(&result, &snapshot.id);
            let file_path = report_dir.join("report.md");
            std::fs::write(&file_path, &content)?;
            println!("{}", content);
            println!("Report saved to {}", file_path.display());
        }
        _ => {
            anyhow::bail!("Unknown format: {}. Use 'json' or 'md'.", format);
        }
    }

    Ok(())
}
