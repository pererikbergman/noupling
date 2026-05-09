use std::path::Path;

pub fn run(path: &str, last: usize, by_module: bool) -> anyhow::Result<()> {
    let db = super::find_db(path)?;
    let snap_repo = crate::storage::repository::SnapshotRepository::new(&db.conn);
    let module_repo = crate::storage::repository::ModuleRepository::new(&db.conn);
    let dep_repo = crate::storage::repository::DependencyRepository::new(&db.conn);

    let project_settings = crate::settings::Settings::load(Path::new(path))?;
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

    if by_module {
        return run_by_module(
            display_snapshots,
            &module_repo,
            &dep_repo,
            &project_settings,
            snapshots.len(),
        );
    }

    println!(
        "{:<12} {:<22} {:>8} {:>10} {:>10} {:>8}",
        "SNAPSHOT", "TIMESTAMP", "SCORE", "MODULES", "VIOLATIONS", "DELTA"
    );
    println!("{}", "-".repeat(76));

    let mut prev_score: Option<f64> = None;

    for snap in display_snapshots {
        let modules = module_repo.get_by_snapshot(&snap.id)?;
        let dependencies = dep_repo.get_by_snapshot(&snap.id)?;

        let result =
            crate::analyzer::audit_with_settings(&modules, &dependencies, &project_settings);

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

fn run_by_module(
    snapshots: &[crate::core::Snapshot],
    module_repo: &crate::storage::repository::ModuleRepository,
    dep_repo: &crate::storage::repository::DependencyRepository,
    settings: &crate::settings::Settings,
    total_snapshots: usize,
) -> anyhow::Result<()> {
    use std::collections::{BTreeMap, BTreeSet};

    let mut all_dirs: BTreeSet<String> = BTreeSet::new();
    let mut snapshot_scores: Vec<(String, BTreeMap<String, f64>)> = Vec::new();

    for snap in snapshots {
        let modules = module_repo.get_by_snapshot(&snap.id)?;
        let dependencies = dep_repo.get_by_snapshot(&snap.id)?;

        let result = crate::analyzer::audit_with_settings(&modules, &dependencies, settings);

        let mut dir_severity: BTreeMap<String, f64> = BTreeMap::new();
        let mut dir_modules: BTreeMap<String, usize> = BTreeMap::new();

        for m in &modules {
            if let Some(top) = m.path.split('/').next() {
                if m.path.contains('/') {
                    *dir_modules.entry(top.to_string()).or_insert(0) += 1;
                }
            }
        }

        for v in &result.violations {
            if let Some(top) = v.dir_a.split('/').next() {
                *dir_severity.entry(top.to_string()).or_insert(0.0) += v.severity;
            }
        }

        let mut scores: BTreeMap<String, f64> = BTreeMap::new();
        for (dir, count) in &dir_modules {
            let sev = dir_severity.get(dir).copied().unwrap_or(0.0);
            let score = (100.0 * (1.0 - sev / *count as f64)).max(0.0);
            scores.insert(dir.clone(), score);
            all_dirs.insert(dir.clone());
        }

        let short_id = if snap.id.len() > 8 {
            snap.id[..8].to_string()
        } else {
            snap.id.clone()
        };
        snapshot_scores.push((short_id, scores));
    }

    if all_dirs.is_empty() {
        println!("No modules found across snapshots.");
        return Ok(());
    }

    let dirs: Vec<String> = all_dirs.into_iter().collect();
    print!("{:<12}", "SNAPSHOT");
    for dir in &dirs {
        print!(" {:>12}", dir);
    }
    println!();
    println!("{}", "-".repeat(12 + dirs.len() * 13));

    let mut prev_scores: BTreeMap<String, f64> = BTreeMap::new();
    for (snap_id, scores) in &snapshot_scores {
        print!("{:<12}", snap_id);
        for dir in &dirs {
            let score = scores.get(dir).copied().unwrap_or(0.0);
            let delta = prev_scores
                .get(dir)
                .map(|&prev| score - prev)
                .unwrap_or(0.0);
            let arrow = if delta > 0.5 {
                "+"
            } else if delta < -0.5 {
                "-"
            } else {
                " "
            };
            print!(" {:>10.1}{}", score, arrow);
        }
        println!();
        for dir in &dirs {
            if let Some(&score) = scores.get(dir) {
                prev_scores.insert(dir.clone(), score);
            }
        }
    }

    println!(
        "\nShowing {} of {} snapshots",
        snapshots.len(),
        total_snapshots
    );

    Ok(())
}
