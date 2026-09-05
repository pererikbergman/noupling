use std::path::Path;

pub fn run(
    path: &str,
    snapshot_id: Option<&str>,
    fail_below: Option<f64>,
    use_baseline: bool,
    module_filter: Option<&str>,
) -> anyhow::Result<()> {
    let session = crate::db_session::DatabaseSession::open(path)?;
    let snap_repo = session.snapshots();
    let module_repo = session.modules();
    let dep_repo = session.dependencies();

    let snapshot = match snapshot_id {
        Some(id) => snap_repo
            .get_by_id(id)?
            .ok_or_else(|| anyhow::anyhow!("Snapshot not found: {}", id))?,
        None => snap_repo
            .get_latest()?
            .ok_or_else(|| anyhow::anyhow!("No snapshots found. Run `noupling scan` first."))?,
    };

    let modules = module_repo.get_by_snapshot(&snapshot.id)?;
    let dependencies = dep_repo.get_by_snapshot(&snapshot.id)?;

    let project_settings = noupling_core::settings::Settings::load(Path::new(path))?;

    // Monorepo mode: multiple configured modules
    if !project_settings.modules.is_empty() {
        // Same inputs the single-module pipeline uses, so every module is
        // audited exactly as `audit --module` / `report --module` audit it (#357).
        let type_counts = noupling_core::scanner::recompute_type_counts(Path::new(path), &modules);
        let monorepo = noupling_core::analyzer::audit_modules(
            &modules,
            &dependencies,
            &type_counts,
            &project_settings,
        );

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
            print!("{}", crate::reporter::format_text(result));
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
            print!("{}", crate::reporter::format_monorepo_text(&monorepo));
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

    // Single-project mode — the load → filter → audit → enrich → diff
    // ladder lives in AuditPipeline (#304). This caller owns only the
    // audit-specific extras (diff-mode print, violation age, baseline
    // compare, fail-below).

    // Pre-pipeline: surface the diff-mode banner. The pipeline applies
    // the filter; we just want the user-facing print.
    let pre_meta = snap_repo.get_meta(&snapshot.id)?;
    if pre_meta.diff_changed_files.is_some() {
        if let Some(ref base) = pre_meta.diff_base {
            if !base.is_empty() {
                println!("Diff mode: filtered to changes against {}", base);
            }
        }
    }

    let pipeline =
        crate::audit_pipeline::AuditPipeline::new(Path::new(path), session.db(), &project_settings);
    let outcome = pipeline.run(crate::audit_pipeline::PipelineOptions {
        snapshot_id: Some(&snapshot.id),
        module_filter,
        baseline: use_baseline,
    })?;
    // Persist the score and per-kind Issue counts on the snapshot row so
    // the Explorer's history scrubber and the strategy report can trend
    // them without re-auditing. Skipped for partial (diff / --module) runs.
    crate::audit_pipeline::record_snapshot_trends(&snap_repo, &outcome);
    let crate::audit_pipeline::PipelineOutcome {
        mut result,
        baseline: baseline_info,
        ..
    } = outcome;

    // Compute violation age from snapshot history. Specific to audit;
    // not part of the shared pipeline.
    let all_snapshots = snap_repo.get_all()?;
    let mut historical: Vec<Vec<(String, String)>> = Vec::new();
    for s in &all_snapshots {
        if s.id == snapshot.id {
            continue;
        }
        let s_mods = module_repo.get_by_snapshot(&s.id)?;
        let s_deps = dep_repo.get_by_snapshot(&s.id)?;
        let s_result = noupling_core::analyzer::audit(&s_mods, &s_deps);
        let fingerprints: Vec<(String, String)> = s_result
            .issue_violations()
            .into_iter()
            .map(noupling_core::analyzer::age_key)
            .collect();
        historical.push(fingerprints);
    }
    result.violation_age =
        noupling_core::analyzer::compute_violation_age(&result.issue_violations(), &historical);

    print!("{}", crate::reporter::format_text(&result));

    // Exit-code contract: fail only on Issues the baseline does not accept.
    if let Some(cmp) = baseline_info {
        println!("\nBaseline comparison:");
        println!("  Not in baseline: {}", cmp.new_count);
        println!("  Baselined: {}", cmp.baselined_count);
        println!(
            "  Resolved (in baseline, no longer present): {}",
            cmp.resolved_count
        );
        if cmp.new_count > 0 {
            anyhow::bail!(
                "{} issue(s) not in the baseline — fix them or accept them with `noupling baseline save`",
                cmp.new_count
            );
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
