mod analyzer;
mod cli;
mod core;
mod diff;
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
        | Commands::Audit { path, .. }
        | Commands::Report { path, .. } => {
            let settings_path = Path::new(path).join(".noupling").join("settings.json");
            if !settings_path.exists() {
                let _ = settings::Settings::write_defaults(Path::new(path));
            }
        }
    }

    let result = match cli.command {
        Commands::Init { path } => run_init(&path),
        Commands::Scan { path, diff_base } => run_scan(&path, diff_base.as_deref()),
        Commands::Audit {
            path,
            snapshot,
            fail_below,
        } => run_audit(&path, snapshot.as_deref(), fail_below),
        Commands::Report { path, format } => run_report(&path, &format),
    };

    if let Err(e) = result {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}

fn run_init(path: &str) -> anyhow::Result<()> {
    let project_path = Path::new(path);
    settings::Settings::write_defaults(project_path)?;
    let settings_path = project_path.join(".noupling").join("settings.json");
    println!("Created {}", settings_path.display());
    println!("Edit this file to customize thresholds, ignored directories, and source extensions.");
    Ok(())
}

/// Load diff metadata if a diff scan was performed.
fn load_diff_meta(path: &str) -> Option<Vec<String>> {
    let meta_path = Path::new(path).join(".noupling").join("diff-meta.json");
    if !meta_path.exists() {
        return None;
    }
    let content = std::fs::read_to_string(&meta_path).ok()?;
    let meta: serde_json::Value = serde_json::from_str(&content).ok()?;
    let files = meta["changed_files"]
        .as_array()?
        .iter()
        .filter_map(|v| v.as_str().map(|s| s.to_string()))
        .collect();
    let base = meta["diff_base"].as_str().unwrap_or("");
    if !base.is_empty() {
        println!("Diff mode: filtered to changes against {}", base);
    }
    Some(files)
}

fn find_db(project_path: &str) -> anyhow::Result<storage::Database> {
    let db_path = Path::new(project_path).join(".noupling").join("history.db");
    if !db_path.exists() {
        anyhow::bail!(
            "No database found at {}. Run `noupling scan <PATH>` first.",
            db_path.display()
        );
    }
    storage::Database::open(&db_path)
}

fn run_scan(path: &str, diff_base: Option<&str>) -> anyhow::Result<()> {
    let project_path = Path::new(path);
    if !project_path.exists() {
        anyhow::bail!("Path does not exist: {}", path);
    }

    // Get changed files if diff mode
    let changed_files = if let Some(base) = diff_base {
        let files = diff::get_changed_files(project_path, base)?;
        println!(
            "Diff mode: {} files changed compared to {}",
            files.len(),
            base
        );
        Some(files)
    } else {
        None
    };

    println!("Scanning: {}", path);

    let db_path = project_path.join(".noupling").join("history.db");
    let db = storage::Database::open(&db_path)?;

    let snap_repo = storage::repository::SnapshotRepository::new(&db.conn);
    let snapshot = snap_repo.create(path)?;
    println!("Created snapshot: {}", snapshot.id);

    // Always scan the full project (needed for dependency resolution)
    let result = scanner::scan_project(project_path, &snapshot.id)?;
    println!("Discovered {} modules", result.modules.len());

    let module_repo = storage::repository::ModuleRepository::new(&db.conn);
    module_repo.bulk_insert(&result.modules)?;

    let mut unique_deps = result.dependencies;
    unique_deps.sort_by(|a, b| {
        (&a.from_module_id, &a.to_module_id, &a.line_number).cmp(&(
            &b.from_module_id,
            &b.to_module_id,
            &b.line_number,
        ))
    });
    unique_deps.dedup_by(|a, b| {
        a.from_module_id == b.from_module_id
            && a.to_module_id == b.to_module_id
            && a.line_number == b.line_number
    });

    let dep_repo = storage::repository::DependencyRepository::new(&db.conn);
    dep_repo.bulk_insert(&unique_deps)?;
    println!("Found {} dependencies", unique_deps.len());

    // Store diff metadata alongside the snapshot
    if let Some(ref files) = changed_files {
        let diff_meta = serde_json::json!({
            "diff_base": diff_base.unwrap_or(""),
            "changed_files": files,
            "changed_count": files.len(),
        });
        let meta_path = project_path.join(".noupling").join("diff-meta.json");
        std::fs::write(&meta_path, serde_json::to_string_pretty(&diff_meta)?)?;
    } else {
        // Remove old diff metadata if doing a full scan
        let meta_path = project_path.join(".noupling").join("diff-meta.json");
        let _ = std::fs::remove_file(&meta_path);
    }

    println!("Scan complete. Database: {}", db_path.display());
    Ok(())
}

fn run_audit(path: &str, snapshot_id: Option<&str>, fail_below: Option<f64>) -> anyhow::Result<()> {
    let db = find_db(path)?;
    let snap_repo = storage::repository::SnapshotRepository::new(&db.conn);

    let snapshot = match snapshot_id {
        Some(id) => snap_repo
            .get_by_id(id)?
            .ok_or_else(|| anyhow::anyhow!("Snapshot not found: {}", id))?,
        None => snap_repo
            .get_latest()?
            .ok_or_else(|| anyhow::anyhow!("No snapshots found. Run `noupling scan` first."))?,
    };

    let module_repo = storage::repository::ModuleRepository::new(&db.conn);
    let dep_repo = storage::repository::DependencyRepository::new(&db.conn);

    let modules = module_repo.get_by_snapshot(&snapshot.id)?;
    let dependencies = dep_repo.get_by_snapshot(&snapshot.id)?;

    let project_settings = settings::Settings::load(Path::new(path))?;
    let mut result = analyzer::audit(&modules, &dependencies);
    result.filter_by_severity(project_settings.thresholds.minimum_severity);

    // Apply diff filter if a diff scan was performed
    if let Some(changed_files) = load_diff_meta(path) {
        result.filter_by_changed_files(&changed_files);
    }

    print!("{}", reporter::format_text(&result));

    if let Some(threshold) = fail_below {
        if result.score < threshold {
            anyhow::bail!(
                "Health score {:.1} is below threshold {:.1}",
                result.score,
                threshold
            );
        }
    }

    Ok(())
}

fn run_report(path: &str, format: &str) -> anyhow::Result<()> {
    let db = find_db(path)?;
    let snap_repo = storage::repository::SnapshotRepository::new(&db.conn);

    let snapshot = snap_repo
        .get_latest()?
        .ok_or_else(|| anyhow::anyhow!("No snapshots found. Run `noupling scan` first."))?;

    let module_repo = storage::repository::ModuleRepository::new(&db.conn);
    let dep_repo = storage::repository::DependencyRepository::new(&db.conn);

    let modules = module_repo.get_by_snapshot(&snapshot.id)?;
    let dependencies = dep_repo.get_by_snapshot(&snapshot.id)?;

    let project_settings = settings::Settings::load(Path::new(path))?;
    let mut result = analyzer::audit(&modules, &dependencies);
    result.filter_by_severity(project_settings.thresholds.minimum_severity);

    // Apply diff filter if a diff scan was performed
    if let Some(changed_files) = load_diff_meta(path) {
        result.filter_by_changed_files(&changed_files);
    }

    let report_dir = Path::new(path).join(".noupling");
    std::fs::create_dir_all(&report_dir)?;

    match format {
        "json" => {
            let report = reporter::JsonReport::from_audit(&modules, &result, &snapshot.id);
            let content = report.to_json()?;
            let file_path = report_dir.join("report.json");
            std::fs::write(&file_path, &content)?;
            println!("Report saved to {}", file_path.display());
        }
        "md" => {
            let md_dir = report_dir.join("report-md");
            reporter::generate_markdown_report(&modules, &result, &snapshot.id, &md_dir)?;
            println!("Report saved to {}/README.md", md_dir.display());
        }
        "xml" => {
            let content = reporter::format_xml(&modules, &result, &snapshot.id);
            let file_path = report_dir.join("report.xml");
            std::fs::write(&file_path, &content)?;
            println!("Report saved to {}", file_path.display());
        }
        "sonar" => {
            let content = reporter::format_sonar(&result);
            let file_path = report_dir.join("noupling-sonar.json");
            std::fs::write(&file_path, &content)?;
            println!("Report saved to {}", file_path.display());
            println!(
                "Add to sonar-project.properties: sonar.externalIssuesReportPaths={}",
                file_path.display()
            );
        }
        "html" => {
            let html_dir = report_dir.join("report");
            reporter::generate_html_report(
                &modules,
                &result,
                &snapshot.id,
                &html_dir,
                &project_settings,
            )?;
            println!("Report saved to {}/index.html", html_dir.display());
        }
        _ => {
            anyhow::bail!(
                "Unknown format: {}. Use 'json', 'xml', 'md', 'html', or 'sonar'.",
                format
            );
        }
    }

    Ok(())
}
