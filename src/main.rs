mod analyzer;
mod baseline;
mod cli;
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
        Commands::Init { path } => run_init(&path),
        Commands::Hook { action, path } => run_hook(&action, &path),
        Commands::Scan { path, diff_base } => run_scan(&path, diff_base.as_deref()),
        Commands::Baseline { action, path } => run_baseline(&action, &path),
        Commands::Audit {
            path,
            snapshot,
            fail_below,
            baseline,
            module,
        } => run_audit(
            &path,
            snapshot.as_deref(),
            fail_below,
            baseline,
            module.as_deref(),
        ),
        Commands::Trend { path, last } => run_trend(&path, last),
        Commands::Report {
            path,
            format,
            module,
        } => run_report(&path, &format, module.as_deref()),
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

fn load_suppressed_count(path: &str) -> usize {
    let meta_path = Path::new(path).join(".noupling").join("suppressed.json");
    let content = std::fs::read_to_string(&meta_path).ok();
    content
        .and_then(|c| serde_json::from_str::<serde_json::Value>(&c).ok())
        .and_then(|v| v["suppressed_count"].as_u64())
        .unwrap_or(0) as usize
}

fn run_hook(action: &str, path: &str) -> anyhow::Result<()> {
    match action {
        "install" => hook::install(Path::new(path)),
        "uninstall" => hook::uninstall(Path::new(path)),
        _ => anyhow::bail!(
            "Unknown hook action: {}. Use 'install' or 'uninstall'.",
            action
        ),
    }
}

fn run_trend(path: &str, last: usize) -> anyhow::Result<()> {
    let db = find_db(path)?;
    let snap_repo = storage::repository::SnapshotRepository::new(&db.conn);
    let module_repo = storage::repository::ModuleRepository::new(&db.conn);
    let dep_repo = storage::repository::DependencyRepository::new(&db.conn);

    let project_settings = settings::Settings::load(Path::new(path))?;
    let snapshots = snap_repo.get_all()?;

    if snapshots.is_empty() {
        println!("No snapshots found. Run `noupling scan` first.");
        return Ok(());
    }

    let display_snapshots = if snapshots.len() > last {
        &snapshots[snapshots.len() - last..]
    } else {
        &snapshots
    };

    println!(
        "{:<12} {:<22} {:>8} {:>10} {:>10} {:>8}",
        "SNAPSHOT", "TIMESTAMP", "SCORE", "MODULES", "VIOLATIONS", "DELTA"
    );
    println!("{}", "-".repeat(76));

    let mut prev_score: Option<f64> = None;

    for snap in display_snapshots {
        let modules = module_repo.get_by_snapshot(&snap.id)?;
        let dependencies = dep_repo.get_by_snapshot(&snap.id)?;

        let mut result = analyzer::audit(&modules, &dependencies);
        result.filter_by_severity(project_settings.thresholds.minimum_severity);
        result.filter_by_layers(&project_settings.layers);

        let delta = match prev_score {
            Some(prev) => {
                let d = result.score - prev;
                if d > 0.0 {
                    format!("+{:.1}", d)
                } else if d < 0.0 {
                    format!("{:.1}", d)
                } else {
                    "0.0".to_string()
                }
            }
            None => "-".to_string(),
        };

        let short_id = if snap.id.len() > 8 {
            &snap.id[..8]
        } else {
            &snap.id
        };

        println!(
            "{:<12} {:<22} {:>7.1} {:>10} {:>10} {:>8}",
            short_id,
            snap.timestamp,
            result.score,
            result.total_modules,
            result.violations.len(),
            delta,
        );

        prev_score = Some(result.score);
    }

    println!(
        "\nShowing {} of {} snapshots",
        display_snapshots.len(),
        snapshots.len()
    );

    Ok(())
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
    let scan_settings = settings::Settings::load(project_path)?;
    let result = scanner::scan_project(
        project_path,
        &snapshot.id,
        scan_settings.allow_inline_suppression,
    )?;
    println!("Discovered {} modules", result.modules.len());
    if result.suppressed_count > 0 {
        println!(
            "{} import{} suppressed by noupling:ignore comments",
            result.suppressed_count,
            if result.suppressed_count == 1 {
                ""
            } else {
                "s"
            }
        );
    }

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

    // Store suppressed count for audit/report
    let suppressed_path = project_path.join(".noupling").join("suppressed.json");
    if result.suppressed_count > 0 {
        let meta = serde_json::json!({ "suppressed_count": result.suppressed_count });
        std::fs::write(&suppressed_path, serde_json::to_string(&meta)?)?;
    } else {
        let _ = std::fs::remove_file(&suppressed_path);
    }

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

fn run_baseline(action: &str, path: &str) -> anyhow::Result<()> {
    match action {
        "save" => {
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
            result.filter_by_layers(&project_settings.layers);

            baseline::save_baseline(Path::new(path), &result)?;
        }
        _ => {
            anyhow::bail!("Unknown baseline action: {}. Use 'save'.", action);
        }
    }
    Ok(())
}

fn run_audit(
    path: &str,
    snapshot_id: Option<&str>,
    fail_below: Option<f64>,
    use_baseline: bool,
    module_filter: Option<&str>,
) -> anyhow::Result<()> {
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

    // Monorepo mode: multiple configured modules
    if !project_settings.modules.is_empty() {
        let monorepo = analyzer::audit_modules(&modules, &dependencies, &project_settings.modules);

        if let Some(name) = module_filter {
            // Single module output
            let (_, result) = monorepo
                .module_results
                .iter()
                .find(|(n, _)| n == name)
                .ok_or_else(|| {
                    anyhow::anyhow!(
                        "Module '{}' not found. Available: {}",
                        name,
                        monorepo
                            .module_results
                            .iter()
                            .map(|(n, _)| n.as_str())
                            .collect::<Vec<_>>()
                            .join(", ")
                    )
                })?;
            print!("{}", reporter::format_text(result));
            if let Some(threshold) = fail_below {
                if result.score < threshold {
                    anyhow::bail!(
                        "Module '{}' score {:.1} is below threshold {:.1}",
                        name,
                        result.score,
                        threshold
                    );
                }
            }
        } else {
            // Multi-module summary
            print!("{}", reporter::format_monorepo_text(&monorepo));
            if let Some(threshold) = fail_below {
                if monorepo.overall_score < threshold {
                    anyhow::bail!(
                        "Overall score {:.1} is below threshold {:.1}",
                        monorepo.overall_score,
                        threshold
                    );
                }
            }
        }
        return Ok(());
    }

    // Single-project mode (existing behavior)
    let mut result = analyzer::audit(&modules, &dependencies);
    result.filter_by_severity(project_settings.thresholds.minimum_severity);
    result.filter_by_layers(&project_settings.layers);
    result.rule_violations = analyzer::check_dependency_rules(
        &modules,
        &dependencies,
        &project_settings.dependency_rules,
    );
    result.layer_violations =
        analyzer::check_layer_rules(&modules, &dependencies, &project_settings.layers);

    // Load suppressed count from scan
    result.suppressed_count = load_suppressed_count(path);

    // Apply diff filter if a diff scan was performed
    if let Some(changed_files) = load_diff_meta(path) {
        result.filter_by_changed_files(&changed_files);
    }

    // Apply baseline filter
    let baseline_info = if use_baseline {
        let (new_count, resolved_count) = baseline::compare_baseline(Path::new(path), &mut result)?;
        Some((new_count, resolved_count))
    } else {
        None
    };

    print!("{}", reporter::format_text(&result));

    if let Some((new_count, resolved_count)) = baseline_info {
        println!("\nBaseline comparison:");
        println!("  New violations: {}", new_count);
        println!("  Resolved violations: {}", resolved_count);
        if new_count > 0 {
            anyhow::bail!("{} new violation(s) introduced since baseline", new_count);
        }
    }

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

fn run_report(path: &str, format: &str, module_filter: Option<&str>) -> anyhow::Result<()> {
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

    // If --module specified with monorepo config, filter to that module's files
    let (report_modules, report_deps) = if let Some(name) = module_filter {
        let cfg = project_settings
            .modules
            .iter()
            .find(|m| m.name == name)
            .ok_or_else(|| anyhow::anyhow!("Module '{}' not found in settings", name))?;
        let prefix = format!("{}/", cfg.path);
        let filtered_modules: Vec<_> = modules
            .iter()
            .filter(|m| m.path.starts_with(&prefix) || m.path == cfg.path)
            .cloned()
            .collect();
        let module_ids: std::collections::HashSet<&str> =
            filtered_modules.iter().map(|m| m.id.as_str()).collect();
        let filtered_deps: Vec<_> = dependencies
            .iter()
            .filter(|d| {
                module_ids.contains(d.from_module_id.as_str())
                    && module_ids.contains(d.to_module_id.as_str())
            })
            .cloned()
            .collect();
        (filtered_modules, filtered_deps)
    } else {
        (modules, dependencies)
    };

    let mut result = analyzer::audit(&report_modules, &report_deps);
    result.filter_by_severity(project_settings.thresholds.minimum_severity);
    result.filter_by_layers(&project_settings.layers);
    result.rule_violations = analyzer::check_dependency_rules(
        &report_modules,
        &report_deps,
        &project_settings.dependency_rules,
    );
    result.layer_violations =
        analyzer::check_layer_rules(&report_modules, &report_deps, &project_settings.layers);

    // Load suppressed count from scan
    result.suppressed_count = load_suppressed_count(path);

    // Apply diff filter if a diff scan was performed
    if let Some(changed_files) = load_diff_meta(path) {
        result.filter_by_changed_files(&changed_files);
    }

    let report_dir = Path::new(path).join(".noupling");
    std::fs::create_dir_all(&report_dir)?;

    match format {
        "json" => {
            let report = reporter::JsonReport::from_audit(&report_modules, &result, &snapshot.id);
            let content = report.to_json()?;
            let file_path = report_dir.join("report.json");
            std::fs::write(&file_path, &content)?;
            println!("Report saved to {}", file_path.display());
        }
        "md" => {
            let md_dir = report_dir.join("report-md");
            reporter::generate_markdown_report(&report_modules, &result, &snapshot.id, &md_dir)?;
            println!("Report saved to {}/README.md", md_dir.display());
        }
        "xml" => {
            let content = reporter::format_xml(&report_modules, &result, &snapshot.id);
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
                &report_modules,
                &result,
                &snapshot.id,
                &html_dir,
                &project_settings,
            )?;
            println!("Report saved to {}/index.html", html_dir.display());
        }
        "mermaid" => {
            let content = reporter::format_mermaid(&report_modules, &result);
            let file_path = report_dir.join("report.mermaid");
            std::fs::write(&file_path, &content)?;
            println!("Report saved to {}", file_path.display());
        }
        "dot" => {
            let content = reporter::format_dot(&report_modules, &result);
            let file_path = report_dir.join("report.dot");
            std::fs::write(&file_path, &content)?;
            println!("Report saved to {}", file_path.display());
            println!(
                "Render with: dot -Tpng {} -o graph.png",
                file_path.display()
            );
        }
        "bundle" => {
            let file_path = report_dir.join("bundle.html");
            reporter::generate_bundle_report(&report_modules, &report_deps, &result, &file_path)?;
            println!("Report saved to {}", file_path.display());
        }
        _ => {
            anyhow::bail!(
                "Unknown format: {}. Use 'json', 'xml', 'md', 'html', 'sonar', 'mermaid', 'dot', or 'bundle'.",
                format
            );
        }
    }

    Ok(())
}
