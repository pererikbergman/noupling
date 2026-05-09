use std::path::Path;

pub fn run(
    path: &str,
    format: &str,
    module_filter: Option<&str>,
    last: usize,
) -> anyhow::Result<()> {
    let db = super::find_db(path)?;
    let snap_repo = crate::storage::repository::SnapshotRepository::new(&db.conn);

    let snapshot = snap_repo
        .get_latest()?
        .ok_or_else(|| anyhow::anyhow!("No snapshots found. Run `noupling scan` first."))?;

    let module_repo = crate::storage::repository::ModuleRepository::new(&db.conn);
    let dep_repo = crate::storage::repository::DependencyRepository::new(&db.conn);

    let modules = module_repo.get_by_snapshot(&snapshot.id)?;
    let dependencies = dep_repo.get_by_snapshot(&snapshot.id)?;

    let project_settings = crate::settings::Settings::load(Path::new(path))?;

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

    let mut result =
        crate::analyzer::audit_with_settings(&report_modules, &report_deps, &project_settings);
    result.rule_violations = crate::analyzer::check_dependency_rules(
        &report_modules,
        &report_deps,
        &project_settings.dependency_rules,
    );
    result.layer_violations =
        crate::analyzer::check_layer_rules(&report_modules, &report_deps, &project_settings.layers);

    // Load scan-time metadata from SQLite
    let scan_meta = snap_repo.get_meta(&snapshot.id)?;
    result.suppressed_count = scan_meta.suppressed_count;
    result.external_deps = scan_meta
        .external_deps
        .iter()
        .map(|e| crate::analyzer::ExternalDepMetric {
            module_path: e.module_path.clone(),
            count: e.count,
        })
        .collect();
    result.total_external_imports = result.external_deps.iter().map(|e| e.count).sum();

    // Apply diff filter if a diff scan was performed
    if let Some(ref changed_files) = scan_meta.diff_changed_files {
        result.filter_by_changed_files(changed_files);
    }

    let report_dir = Path::new(path).join(".noupling");
    std::fs::create_dir_all(&report_dir)?;

    match format {
        "json" => {
            let report =
                crate::reporter::JsonReport::from_audit(&report_modules, &result, &snapshot.id);
            let content = report.to_json()?;
            let file_path = report_dir.join("report.json");
            std::fs::write(&file_path, &content)?;
            println!("Report saved to {}", file_path.display());
        }
        "md" => {
            let md_dir = report_dir.join("report-md");
            crate::reporter::generate_markdown_report(
                &report_modules,
                &result,
                &snapshot.id,
                &md_dir,
            )?;
            println!("Report saved to {}/README.md", md_dir.display());
        }
        "xml" => {
            let content = crate::reporter::format_xml(&report_modules, &result, &snapshot.id);
            let file_path = report_dir.join("report.xml");
            std::fs::write(&file_path, &content)?;
            println!("Report saved to {}", file_path.display());
        }
        "sonar" => {
            let content = crate::reporter::format_sonar(&result);
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
            crate::reporter::generate_html_report(
                &report_modules,
                &result,
                &snapshot.id,
                &html_dir,
                &project_settings,
            )?;
            println!("Report saved to {}/index.html", html_dir.display());
        }
        "mermaid" => {
            let content = crate::reporter::format_mermaid(&report_modules, &result);
            let file_path = report_dir.join("report.mermaid");
            std::fs::write(&file_path, &content)?;
            println!("Report saved to {}", file_path.display());
        }
        "dot" => {
            let content = crate::reporter::format_dot(&report_modules, &result);
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
            crate::reporter::generate_bundle_report(
                &report_modules,
                &report_deps,
                &result,
                &file_path,
            )?;
            println!("Report saved to {}", file_path.display());
        }
        "dashboard" => {
            let file_path = report_dir.join("dashboard.html");
            crate::reporter::generate_dashboard(
                &report_modules,
                &report_deps,
                &result,
                &file_path,
            )?;
            println!("Report saved to {}", file_path.display());
        }
        "pr" => {
            // Compute deltas from previous snapshot if available
            let snap_repo = crate::storage::repository::SnapshotRepository::new(&db.conn);
            let all = snap_repo.get_all()?;
            let prev = all.iter().rfind(|s| s.id != snapshot.id).cloned();
            let (prev_score, prev_count) = if let Some(prev_snap) = prev {
                let prev_modules = module_repo.get_by_snapshot(&prev_snap.id)?;
                let prev_deps = dep_repo.get_by_snapshot(&prev_snap.id)?;
                let prev_result = crate::analyzer::audit_with_settings(
                    &prev_modules,
                    &prev_deps,
                    &project_settings,
                );
                (Some(prev_result.score), Some(prev_result.violations.len()))
            } else {
                (None, None)
            };

            let content = crate::reporter::format_pr(&result, prev_score, prev_count, None, None);
            let file_path = report_dir.join("pr.md");
            std::fs::write(&file_path, &content)?;
            println!("Report saved to {}", file_path.display());
        }
        "briefing" => {
            let content = crate::reporter::format_briefing(&result);
            let file_path = report_dir.join("briefing.md");
            std::fs::write(&file_path, &content)?;
            println!("Report saved to {}", file_path.display());
        }
        "strategy" => {
            let snap_repo = crate::storage::repository::SnapshotRepository::new(&db.conn);
            let file_path = report_dir.join("strategy.html");
            crate::reporter::generate_strategy_report(
                &snap_repo,
                &module_repo,
                &dep_repo,
                &project_settings,
                last,
                &file_path,
            )?;
            println!("Report saved to {}", file_path.display());
        }
        "all" => {
            let formats = [
                "json",
                "xml",
                "md",
                "html",
                "sonar",
                "mermaid",
                "dot",
                "bundle",
                "dashboard",
                "pr",
                "briefing",
            ];
            let mut succeeded = 0;
            let mut failed = 0;
            for f in formats {
                let r = generate_single_format(
                    f,
                    &report_dir,
                    &report_modules,
                    &report_deps,
                    &result,
                    &snapshot.id,
                    &project_settings,
                );
                match r {
                    Ok(()) => succeeded += 1,
                    Err(e) => {
                        eprintln!("Warning: failed to generate '{}' report: {}", f, e);
                        failed += 1;
                    }
                }
            }
            // Strategy needs snapshot history — handle separately
            let snap_repo = crate::storage::repository::SnapshotRepository::new(&db.conn);
            let strategy_path = report_dir.join("strategy.html");
            match crate::reporter::generate_strategy_report(
                &snap_repo,
                &module_repo,
                &dep_repo,
                &project_settings,
                last,
                &strategy_path,
            ) {
                Ok(()) => {
                    succeeded += 1;
                    println!("Report saved to {}", strategy_path.display());
                }
                Err(e) => {
                    eprintln!("Warning: failed to generate 'strategy' report: {}", e);
                    failed += 1;
                }
            }
            println!(
                "\nGenerated {} report(s){}",
                succeeded,
                if failed > 0 {
                    format!(" ({} failed)", failed)
                } else {
                    String::new()
                }
            );
        }
        _ => {
            anyhow::bail!(
                "Unknown format: {}. Use 'json', 'xml', 'md', 'html', 'sonar', 'mermaid', 'dot', 'bundle', 'dashboard', 'pr', 'briefing', 'strategy', or 'all'.",
                format
            );
        }
    }

    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn generate_single_format(
    format: &str,
    report_dir: &Path,
    modules: &[crate::core::Module],
    deps: &[crate::core::Dependency],
    result: &crate::analyzer::AuditResult,
    snapshot_id: &str,
    settings: &crate::settings::Settings,
) -> anyhow::Result<()> {
    match format {
        "json" => {
            let report = crate::reporter::JsonReport::from_audit(modules, result, snapshot_id);
            let content = report.to_json()?;
            let file_path = report_dir.join("report.json");
            std::fs::write(&file_path, &content)?;
            println!("Report saved to {}", file_path.display());
        }
        "md" => {
            let md_dir = report_dir.join("report-md");
            crate::reporter::generate_markdown_report(modules, result, snapshot_id, &md_dir)?;
            println!("Report saved to {}/README.md", md_dir.display());
        }
        "xml" => {
            let content = crate::reporter::format_xml(modules, result, snapshot_id);
            let file_path = report_dir.join("report.xml");
            std::fs::write(&file_path, &content)?;
            println!("Report saved to {}", file_path.display());
        }
        "sonar" => {
            let content = crate::reporter::format_sonar(result);
            let file_path = report_dir.join("noupling-sonar.json");
            std::fs::write(&file_path, &content)?;
            println!("Report saved to {}", file_path.display());
        }
        "html" => {
            let html_dir = report_dir.join("report");
            crate::reporter::generate_html_report(
                modules,
                result,
                snapshot_id,
                &html_dir,
                settings,
            )?;
            println!("Report saved to {}/index.html", html_dir.display());
        }
        "mermaid" => {
            let content = crate::reporter::format_mermaid(modules, result);
            let file_path = report_dir.join("report.mermaid");
            std::fs::write(&file_path, &content)?;
            println!("Report saved to {}", file_path.display());
        }
        "dot" => {
            let content = crate::reporter::format_dot(modules, result);
            let file_path = report_dir.join("report.dot");
            std::fs::write(&file_path, &content)?;
            println!("Report saved to {}", file_path.display());
        }
        "bundle" => {
            let file_path = report_dir.join("bundle.html");
            crate::reporter::generate_bundle_report(modules, deps, result, &file_path)?;
            println!("Report saved to {}", file_path.display());
        }
        "dashboard" => {
            let file_path = report_dir.join("dashboard.html");
            crate::reporter::generate_dashboard(modules, deps, result, &file_path)?;
            println!("Report saved to {}", file_path.display());
        }
        "pr" => {
            // Without snapshot history context, generate a simple current-state PR report.
            let content = crate::reporter::format_pr(result, None, None, None, None);
            let file_path = report_dir.join("pr.md");
            std::fs::write(&file_path, &content)?;
            println!("Report saved to {}", file_path.display());
        }
        "briefing" => {
            let content = crate::reporter::format_briefing(result);
            let file_path = report_dir.join("briefing.md");
            std::fs::write(&file_path, &content)?;
            println!("Report saved to {}", file_path.display());
        }
        _ => anyhow::bail!("unknown format"),
    }
    Ok(())
}
