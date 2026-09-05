use std::path::Path;

#[allow(clippy::too_many_arguments)]
pub fn run(
    path: &str,
    format: &str,
    module_filter: Option<&str>,
    last: usize,
    explorer_output: Option<&str>,
    explorer_editor: Option<&str>,
    explorer_title: Option<&str>,
    explorer_no_history: bool,
    use_baseline: bool,
) -> anyhow::Result<()> {
    let session = crate::db_session::DatabaseSession::open(path)?;
    let snap_repo = session.snapshots();
    let module_repo = session.modules();
    let dep_repo = session.dependencies();
    let project_settings = noupling_core::settings::Settings::load(Path::new(path))?;

    // The shared load → filter → audit → enrich → diff-filter ladder
    // is in the AuditPipeline (#304). This caller owns only the bits
    // specific to `report` (trend persistence + format dispatch).
    let pipeline =
        crate::audit_pipeline::AuditPipeline::new(Path::new(path), session.db(), &project_settings);
    let crate::audit_pipeline::PipelineOutcome {
        snapshot,
        modules: report_modules,
        dependencies: report_deps,
        result,
        ..
    } = pipeline.run(crate::audit_pipeline::PipelineOptions {
        snapshot_id: None,
        module_filter,
        baseline: use_baseline,
    })?;

    // Persist the score and per-kind Issue counts on the snapshot row so
    // the Explorer's history scrubber and the strategy report can trend
    // them, even when the user only ever runs `noupling report`.
    crate::audit_pipeline::record_snapshot_trends(&snap_repo, &snapshot.id, &result);

    let report_dir = Path::new(path).join(".noupling");
    std::fs::create_dir_all(&report_dir)?;

    // Eleven of the thirteen formats run through one registry
    // (#317 widens the #301 seam). Only `strategy` (needs the
    // session/repo triad for its history walk) and `explorer`
    // (carries an option struct that would balloon
    // FormatterContext for no gain) keep bespoke arms.
    let (prev_score, prev_violation_count) = previous_snapshot_deltas(
        &snap_repo,
        &module_repo,
        &dep_repo,
        &snapshot,
        &project_settings,
    );
    let registry = crate::report_formatter::builtin_formatters();
    let ctx = crate::report_formatter::FormatterContext {
        modules: &report_modules,
        deps: &report_deps,
        result: &result,
        snapshot: &snapshot,
        report_dir: &report_dir,
        settings: &project_settings,
        prev_score,
        prev_violation_count,
    };
    if let Some(out) = crate::report_formatter::dispatch(format, &ctx, &registry)? {
        crate::report_formatter::write(&out)?;
        return Ok(());
    }

    match format {
        "explorer" => {
            // Load all prior snapshots with recorded scores so the
            // history scrubber has a trend to render. Cheap: small,
            // indexed SELECT. Returns empty for fresh projects.
            let history: Vec<noupling_explorer::HistoryEntry> = snap_repo
                .get_all_with_scores()
                .unwrap_or_default()
                .into_iter()
                .map(|r| noupling_explorer::HistoryEntry {
                    snapshot_id: r.snapshot_id,
                    taken_at: r.taken_at,
                    health_score: r.health_score,
                })
                .collect();

            // #280: load optional per-module LLM enrichment from
            // .noupling/enrichment/modules.json. Skipped entirely if
            // the file doesn't exist; warn-and-skip on parse errors so
            // a broken sidecar can't break report generation.
            let module_enrichment = load_module_enrichment(Path::new(path));

            let options = noupling_explorer::RenderOptions {
                editor: explorer_editor.map(str::to_string),
                title: explorer_title.map(str::to_string),
                include_history: !explorer_no_history,
                history,
                module_enrichment,
            };
            // Resolve the codebase root to an absolute path so the template's
            // editor URLs (e.g. `vscode://file//Users/me/foo.kt:1`) point at
            // a real file on disk. The Snapshot stored at scan-time may carry
            // a relative path like `.` or `./project`, which is fine for
            // analysis but produces broken editor links.
            let abs_root = std::fs::canonicalize(path)
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|_| snapshot.root_path.clone());
            let resolved_snapshot = noupling_core::core::Snapshot {
                root_path: abs_root,
                ..snapshot.clone()
            };

            // The Explorer is a view over the same AuditResult every other
            // format reads (ADR 0001). Inferred layers and the actionable
            // fallback already happened inside the pipeline; the result
            // carries `layers` + `layers_auto_detected` for the banner.
            let html = noupling_explorer::render(
                &report_modules,
                &report_deps,
                &result,
                &project_settings,
                &resolved_snapshot,
                &options,
            )?;
            let file_path = match explorer_output {
                Some(p) => Path::new(p).to_path_buf(),
                None => report_dir.join("explorer.html"),
            };
            if let Some(parent) = file_path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(&file_path, html)?;
            println!("Report saved to {}", file_path.display());
        }
        "strategy" => {
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
            // Enumerate the registry instead of a hardcoded list — a
            // new adapter (#317 added md/html/bundle/dashboard/pr to
            // the existing six) automatically participates here.
            let mut succeeded = 0usize;
            let mut failed = 0usize;
            for adapter in &registry {
                match adapter.render(&ctx) {
                    Ok(out) => {
                        if let Err(e) = crate::report_formatter::write(&out) {
                            eprintln!(
                                "Warning: failed to write '{}' report: {}",
                                adapter.name(),
                                e
                            );
                            failed += 1;
                        } else {
                            succeeded += 1;
                        }
                    }
                    Err(e) => {
                        eprintln!(
                            "Warning: failed to generate '{}' report: {}",
                            adapter.name(),
                            e
                        );
                        failed += 1;
                    }
                }
            }
            // Strategy needs snapshot history + repos — handle
            // separately. Same bespoke shape as the focused `strategy`
            // arm.
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
                "Unknown format: {}. Use 'text', 'json', 'xml', 'md', 'html', 'sonar', 'mermaid', 'dot', 'bundle', 'dashboard', 'pr', 'briefing', 'strategy', 'explorer', or 'all'.",
                format
            );
        }
    }

    Ok(())
}

/// Compute the previous-snapshot deltas the `pr` adapter needs.
/// Returns `(None, None)` when no prior snapshot exists or the
/// query fails — the PR report falls back to current-state-only
/// rendering. Generic over the repository borrow lifetime so the
/// caller can pass references straight out of `DatabaseSession`.
fn previous_snapshot_deltas<'a>(
    snap_repo: &noupling_core::storage::repository::SnapshotRepository<'a>,
    module_repo: &noupling_core::storage::repository::ModuleRepository<'a>,
    dep_repo: &noupling_core::storage::repository::DependencyRepository<'a>,
    current: &noupling_core::core::Snapshot,
    settings: &noupling_core::settings::Settings,
) -> (Option<f64>, Option<usize>) {
    let all = match snap_repo.get_all() {
        Ok(v) => v,
        Err(_) => return (None, None),
    };
    let Some(prev_snap) = all.iter().rfind(|s| s.id != current.id).cloned() else {
        return (None, None);
    };
    let prev_modules = match module_repo.get_by_snapshot(&prev_snap.id) {
        Ok(v) => v,
        Err(_) => return (None, None),
    };
    let prev_deps = match dep_repo.get_by_snapshot(&prev_snap.id) {
        Ok(v) => v,
        Err(_) => return (None, None),
    };
    let prev_result =
        noupling_core::analyzer::audit_with_settings(&prev_modules, &prev_deps, &[], settings);
    (Some(prev_result.score), Some(prev_result.violations.len()))
}

/// Read per-module LLM enrichment from
/// `.noupling/enrichment/modules.json`. Returns an empty list if the
/// file is absent or unparseable; logs a warning on parse failure so
/// a broken sidecar doesn't block report generation (PR #280).
///
/// Schema:
/// ```json
/// {
///   "schema_version": 1,
///   "entries": [
///     {
///       "module_path": "src/payments",
///       "summary": "Payment processing",
///       "responsibility": "Drives Stripe / Adyen / cash flows…",
///       "tags": ["domain"],
///       "generated_at": "2026-06-05T10:32:01Z",
///       "model": "claude-opus-4-7"
///     }
///   ]
/// }
/// ```
fn load_module_enrichment(
    project_path: &std::path::Path,
) -> Vec<noupling_explorer::ModuleEnrichmentEntry> {
    let path = project_path
        .join(".noupling")
        .join("enrichment")
        .join("modules.json");
    if !path.exists() {
        return Vec::new();
    }
    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!(
                "warning: failed to read {}: {} — Composition view will use derived metadata only",
                path.display(),
                e
            );
            return Vec::new();
        }
    };
    let parsed: serde_json::Value = match serde_json::from_str(&content) {
        Ok(v) => v,
        Err(e) => {
            eprintln!(
                "warning: {} is not valid JSON: {} — Composition view will use derived metadata only",
                path.display(),
                e
            );
            return Vec::new();
        }
    };
    let entries = parsed
        .get("entries")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    entries
        .into_iter()
        .filter_map(|e| {
            let path = e.get("module_path")?.as_str()?.to_string();
            Some(noupling_explorer::ModuleEnrichmentEntry {
                module_path: path,
                summary: e
                    .get("summary")
                    .and_then(|v| v.as_str())
                    .map(str::to_string),
                responsibility: e
                    .get("responsibility")
                    .and_then(|v| v.as_str())
                    .map(str::to_string),
                tags: e
                    .get("tags")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_str().map(str::to_string))
                            .collect()
                    })
                    .unwrap_or_default(),
                generated_at: e
                    .get("generated_at")
                    .and_then(|v| v.as_str())
                    .map(str::to_string),
                model: e.get("model").and_then(|v| v.as_str()).map(str::to_string),
            })
        })
        .collect()
}
