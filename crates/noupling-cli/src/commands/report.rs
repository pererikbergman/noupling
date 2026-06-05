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
) -> anyhow::Result<()> {
    let session = crate::db_session::DatabaseSession::open(path)?;
    let snap_repo = session.snapshots();
    let module_repo = session.modules();
    let dep_repo = session.dependencies();
    let project_settings = noupling_core::settings::Settings::load(Path::new(path))?;

    // The shared load → filter → audit → enrich → diff-filter ladder
    // is in the AuditPipeline (#304). This caller owns only the bits
    // specific to `report` (save_health_score + format dispatch).
    let pipeline =
        crate::audit_pipeline::AuditPipeline::new(Path::new(path), session.db(), &project_settings);
    let crate::audit_pipeline::PipelineOutcome {
        snapshot,
        modules: report_modules,
        dependencies: report_deps,
        result,
    } = pipeline.run(crate::audit_pipeline::PipelineOptions {
        snapshot_id: None,
        module_filter,
    })?;

    // Persist the score on the snapshot row so the Explorer's history
    // scrubber (PRD §10.5) can render a trend over time, even when the
    // user only ever runs `noupling report` (and never `audit`).
    let _ = snap_repo.save_health_score(&snapshot.id, result.score);

    let report_dir = Path::new(path).join(".noupling");
    std::fs::create_dir_all(&report_dir)?;

    // The six simple formats (string + file + print) are now adapters
    // behind a single registry (#301). Anything that matches an
    // adapter returns here; complex formats — markdown, html, bundle,
    // dashboard, pr, explorer, strategy, all — keep their bespoke
    // arms below because their input shapes differ.
    let registry = crate::report_formatter::builtin_formatters();
    let ctx = crate::report_formatter::FormatterContext {
        modules: &report_modules,
        result: &result,
        snapshot: &snapshot,
        report_dir: &report_dir,
    };
    if let Some(out) = crate::report_formatter::dispatch(format, &ctx, &registry)? {
        std::fs::write(&out.file_path, &out.content)?;
        println!("Report saved to {}", out.file_path.display());
        if let Some(tail) = out.success_tail {
            println!("{}", tail);
        }
        return Ok(());
    }

    match format {
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
            // Compute deltas from previous snapshot if available.
            let all = snap_repo.get_all()?;
            let prev = all.iter().rfind(|s| s.id != snapshot.id).cloned();
            let (prev_score, prev_count) = if let Some(prev_snap) = prev {
                let prev_modules = module_repo.get_by_snapshot(&prev_snap.id)?;
                let prev_deps = dep_repo.get_by_snapshot(&prev_snap.id)?;
                let prev_result = noupling_core::analyzer::audit_with_settings(
                    &prev_modules,
                    &prev_deps,
                    &[],
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

            let mut options = noupling_explorer::RenderOptions {
                editor: explorer_editor.map(str::to_string),
                title: explorer_title.map(str::to_string),
                include_history: !explorer_no_history,
                layers_auto_detected: false,
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

            // When the project has no configured layers, infer a sensible
            // set from common path-segment patterns (Android `ui/domain/data`,
            // Spring `controller/service/repository`, etc.) and re-run audit
            // so the Explorer reflects the inferred architecture. The
            // user-written settings.json is left untouched on disk.
            let auto_detected = if project_settings.layers.is_empty() {
                let d = noupling_explorer::detect_layers(&report_modules);
                if d.is_empty() {
                    None
                } else {
                    Some(d)
                }
            } else {
                None
            };

            // Owns the re-audited result when auto-detection fired; left
            // empty otherwise so we can hand the original `result` to the
            // renderer by reference.
            let mut explorer_settings = project_settings.clone();
            let re_audited_holder;
            let result_for_render: &noupling_core::analyzer::AuditResult = if let Some(layers) =
                auto_detected
            {
                options.layers_auto_detected = true;
                explorer_settings.layers = layers;
                // Auto-detected layers are by definition coarse (`**/ui/**`
                // etc.), so every sibling coupling inside one of them gets
                // counted as a strict-mode violation and the score plummets
                // to 0 with no actionable signal. Switch the audit to
                // "actionable" mode so only circular deps count as
                // violations; siblings become informational. The user can
                // override by adding their own `coupling_mode` to settings.
                if explorer_settings.coupling_mode.is_none() {
                    explorer_settings.coupling_mode = Some("actionable".to_string());
                }
                let type_counts =
                    noupling_core::scanner::recompute_type_counts(Path::new(path), &report_modules);
                let mut re_audited = noupling_core::analyzer::audit_with_settings(
                    &report_modules,
                    &report_deps,
                    &type_counts,
                    &explorer_settings,
                );
                re_audited.rule_violations = noupling_core::analyzer::check_dependency_rules(
                    &report_modules,
                    &report_deps,
                    &explorer_settings.dependency_rules,
                );
                re_audited.layer_violations = noupling_core::analyzer::check_layer_rules(
                    &report_modules,
                    &report_deps,
                    &explorer_settings.layers,
                );
                re_audited.suppressed_count = result.suppressed_count;
                re_audited.external_deps = result.external_deps.clone();
                re_audited.total_external_imports = result.total_external_imports;
                re_audited_holder = re_audited;
                &re_audited_holder
            } else {
                &result
            };

            let html = noupling_explorer::render(
                &report_modules,
                &report_deps,
                result_for_render,
                &explorer_settings,
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
                    &snapshot,
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
            // Strategy needs snapshot history — handle separately.
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
                "Unknown format: {}. Use 'json', 'xml', 'md', 'html', 'sonar', 'mermaid', 'dot', 'bundle', 'dashboard', 'pr', 'briefing', 'strategy', 'explorer', or 'all'.",
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
    modules: &[noupling_core::core::Module],
    deps: &[noupling_core::core::Dependency],
    result: &noupling_core::analyzer::AuditResult,
    snapshot: &noupling_core::core::Snapshot,
    settings: &noupling_core::settings::Settings,
) -> anyhow::Result<()> {
    // Try the simple-format registry first — six of the eleven `all`
    // arms (json, xml, sonar, mermaid, dot, briefing) go through it.
    let registry = crate::report_formatter::builtin_formatters();
    let ctx = crate::report_formatter::FormatterContext {
        modules,
        result,
        snapshot,
        report_dir,
    };
    if let Some(out) = crate::report_formatter::dispatch(format, &ctx, &registry)? {
        std::fs::write(&out.file_path, &out.content)?;
        println!("Report saved to {}", out.file_path.display());
        // The success-tail messages are noisy in `all`-mode; suppress
        // them and only print on the focused arms.
        return Ok(());
    }

    // Complex formats that don't fit the simple seam yet.
    match format {
        "md" => {
            let md_dir = report_dir.join("report-md");
            crate::reporter::generate_markdown_report(modules, result, &snapshot.id, &md_dir)?;
            println!("Report saved to {}/README.md", md_dir.display());
        }
        "html" => {
            let html_dir = report_dir.join("report");
            crate::reporter::generate_html_report(
                modules,
                result,
                &snapshot.id,
                &html_dir,
                settings,
            )?;
            println!("Report saved to {}/index.html", html_dir.display());
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
        _ => anyhow::bail!("unknown format"),
    }
    Ok(())
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
