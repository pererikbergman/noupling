use std::path::Path;

pub fn run(
    path: &str,
    snapshot_id: Option<&str>,
    fail_below: Option<f64>,
    use_baseline: bool,
    module_filter: Option<&str>,
) -> anyhow::Result<()> {
    let db = super::find_db(path)?;
    let snap_repo = crate::storage::repository::SnapshotRepository::new(&db.conn);

    let snapshot = match snapshot_id {
        Some(id) => snap_repo
            .get_by_id(id)?
            .ok_or_else(|| anyhow::anyhow!("Snapshot not found: {}", id))?,
        None => snap_repo
            .get_latest()?
            .ok_or_else(|| anyhow::anyhow!("No snapshots found. Run `noupling scan` first."))?,
    };

    let module_repo = crate::storage::repository::ModuleRepository::new(&db.conn);
    let dep_repo = crate::storage::repository::DependencyRepository::new(&db.conn);

    let modules = module_repo.get_by_snapshot(&snapshot.id)?;
    let dependencies = dep_repo.get_by_snapshot(&snapshot.id)?;

    let project_settings = crate::settings::Settings::load(Path::new(path))?;

    // Monorepo mode: multiple configured modules
    if !project_settings.modules.is_empty() {
        let monorepo =
            crate::analyzer::audit_modules(&modules, &dependencies, &project_settings.modules);

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

    // Single-project mode (existing behavior)
    let mut result =
        crate::analyzer::audit_with_settings(&modules, &dependencies, &project_settings);
    result.rule_violations = crate::analyzer::check_dependency_rules(
        &modules,
        &dependencies,
        &project_settings.dependency_rules,
    );
    result.layer_violations =
        crate::analyzer::check_layer_rules(&modules, &dependencies, &project_settings.layers);

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

    // Compute violation age from snapshot history
    let all_snapshots = snap_repo.get_all()?;
    let mut historical: Vec<Vec<(String, String)>> = Vec::new();
    for s in &all_snapshots {
        if s.id == snapshot.id {
            continue;
        }
        let s_mods = module_repo.get_by_snapshot(&s.id)?;
        let s_deps = dep_repo.get_by_snapshot(&s.id)?;
        let s_result = crate::analyzer::audit(&s_mods, &s_deps);
        let fingerprints: Vec<(String, String)> = s_result
            .violations
            .iter()
            .map(|v| (v.from_module.clone(), v.to_module.clone()))
            .collect();
        historical.push(fingerprints);
    }
    result.violation_age = crate::analyzer::compute_violation_age(&result.violations, &historical);

    // Apply diff filter if a diff scan was performed
    if let Some(ref changed_files) = scan_meta.diff_changed_files {
        if let Some(ref base) = scan_meta.diff_base {
            if !base.is_empty() {
                println!("Diff mode: filtered to changes against {}", base);
            }
        }
        result.filter_by_changed_files(changed_files);
    }

    // Apply baseline filter
    let baseline_info = if use_baseline {
        let (new_count, resolved_count) =
            crate::baseline::compare_baseline(Path::new(path), &mut result)?;
        Some((new_count, resolved_count))
    } else {
        None
    };

    print!("{}", crate::reporter::format_text(&result));

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
