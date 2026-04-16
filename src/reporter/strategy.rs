//! Executive strategy report: multi-snapshot trends and trajectory.

use crate::analyzer::{audit, AuditResult};
use crate::core::{Dependency, Module, Snapshot};
use crate::settings::Settings;
use crate::storage::repository::{DependencyRepository, ModuleRepository, SnapshotRepository};
use serde::Serialize;
use std::collections::BTreeMap;

type SnapshotResult = (String, String, AuditResult, Vec<Module>, Vec<Dependency>);

#[derive(Serialize)]
pub struct StrategyData {
    pub snapshots: Vec<SnapshotPoint>,
    pub modules_trajectory: Vec<ModuleTrajectory>,
    pub velocity: Velocity,
    pub risk_concentration: Vec<RiskModule>,
    pub headline: String,
    pub current_score: f64,
}

#[derive(Serialize)]
pub struct SnapshotPoint {
    pub id: String,
    pub timestamp: String,
    pub score: f64,
    pub violations: usize,
    pub total_xs: usize,
}

#[derive(Serialize)]
pub struct ModuleTrajectory {
    pub name: String,
    /// Score per snapshot (in chronological order).
    pub scores: Vec<f64>,
    /// Trend: positive = improving, negative = degrading.
    pub trend: f64,
}

#[derive(Serialize)]
pub struct Velocity {
    pub violations_resolved_avg: f64,
    pub violations_introduced_avg: f64,
    pub net_improvement_rate: f64,
}

#[derive(Serialize)]
pub struct RiskModule {
    pub path: String,
    pub current_blast_radius: usize,
    pub previous_blast_radius: Option<usize>,
    pub direction: String,
}

#[allow(clippy::too_many_arguments)]
pub fn generate_strategy_report(
    snap_repo: &SnapshotRepository,
    module_repo: &ModuleRepository,
    dep_repo: &DependencyRepository,
    settings: &Settings,
    last: usize,
    output_path: &std::path::Path,
) -> anyhow::Result<()> {
    let all_snapshots = snap_repo.get_all()?;
    if all_snapshots.is_empty() {
        anyhow::bail!("No snapshots found. Run `noupling scan` first.");
    }

    let snapshots: Vec<&Snapshot> = if all_snapshots.len() > last {
        all_snapshots[all_snapshots.len() - last..].iter().collect()
    } else {
        all_snapshots.iter().collect()
    };

    // Compute audit per snapshot
    let mut snapshot_results: Vec<SnapshotResult> = Vec::new();
    for snap in &snapshots {
        let modules = module_repo.get_by_snapshot(&snap.id)?;
        let dependencies = dep_repo.get_by_snapshot(&snap.id)?;
        let mut result = audit(&modules, &dependencies);
        result.filter_by_severity(settings.thresholds.minimum_severity);
        result.apply_coupling_mode(&settings.thresholds.coupling_mode);
        result.filter_by_layers(&settings.layers);
        snapshot_results.push((
            snap.id.clone(),
            snap.timestamp.clone(),
            result,
            modules,
            dependencies,
        ));
    }

    let snapshot_points: Vec<SnapshotPoint> = snapshot_results
        .iter()
        .map(|(id, ts, r, _, _)| SnapshotPoint {
            id: short_id(id),
            timestamp: ts.clone(),
            score: r.score,
            violations: r.violations.len(),
            total_xs: r.total_xs,
        })
        .collect();

    // Module trajectories: per-top-level-directory scores per snapshot
    let mut module_scores: BTreeMap<String, Vec<f64>> = BTreeMap::new();
    let snap_count = snapshot_results.len();
    for (idx, (_, _, result, modules, _)) in snapshot_results.iter().enumerate() {
        let dir_scores = compute_per_module_scores(modules, result);
        // Ensure every module that has appeared has an entry (with NaN for missing snapshots)
        for (dir, score) in &dir_scores {
            let entry = module_scores
                .entry(dir.clone())
                .or_insert_with(|| vec![f64::NAN; snap_count]);
            entry[idx] = *score;
        }
    }
    // Pad any modules added later
    for scores in module_scores.values_mut() {
        if scores.len() < snap_count {
            scores.resize(snap_count, f64::NAN);
        }
    }

    let modules_trajectory: Vec<ModuleTrajectory> = module_scores
        .into_iter()
        .map(|(name, scores)| {
            let trend = compute_trend(&scores);
            ModuleTrajectory {
                name,
                scores,
                trend,
            }
        })
        .collect();

    // Velocity: how many violations are added vs removed per snapshot
    let velocity = compute_velocity(&snapshot_results);

    // Risk concentration: top blast-radius modules now vs previous
    let risk_concentration = compute_risk_concentration(&snapshot_results);

    // Headline
    let headline = build_headline(&snapshot_points);
    let current_score = snapshot_points.last().map(|p| p.score).unwrap_or(0.0);

    let data = StrategyData {
        snapshots: snapshot_points,
        modules_trajectory,
        velocity,
        risk_concentration,
        headline,
        current_score,
    };
    let json = serde_json::to_string(&data)?;

    let html = format!(
        include_str!("strategy_template.html"),
        json = json,
        version = super::VERSION,
    );

    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(output_path, html)?;
    Ok(())
}

fn short_id(id: &str) -> String {
    if id.len() > 8 {
        id[..8].to_string()
    } else {
        id.to_string()
    }
}

fn compute_per_module_scores(modules: &[Module], result: &AuditResult) -> BTreeMap<String, f64> {
    let mut dir_modules: BTreeMap<String, usize> = BTreeMap::new();
    for m in modules {
        if let Some(top) = m.path.split('/').next() {
            if m.path.contains('/') {
                *dir_modules.entry(top.to_string()).or_insert(0) += 1;
            }
        }
    }

    let mut dir_severity: BTreeMap<String, f64> = BTreeMap::new();
    for v in &result.violations {
        if let Some(top) = v.dir_a.split('/').next() {
            *dir_severity.entry(top.to_string()).or_insert(0.0) += v.severity;
        }
    }

    let mut scores = BTreeMap::new();
    for (dir, count) in dir_modules {
        let sev = dir_severity.get(&dir).copied().unwrap_or(0.0);
        let score = (100.0 * (1.0 - sev / count.max(1) as f64)).clamp(0.0, 100.0);
        scores.insert(dir, score);
    }
    scores
}

/// Linear regression slope on score series, ignoring NaN values.
fn compute_trend(scores: &[f64]) -> f64 {
    let valid: Vec<(f64, f64)> = scores
        .iter()
        .enumerate()
        .filter(|(_, &s)| !s.is_nan())
        .map(|(i, &s)| (i as f64, s))
        .collect();
    if valid.len() < 2 {
        return 0.0;
    }
    let n = valid.len() as f64;
    let sum_x: f64 = valid.iter().map(|(x, _)| x).sum();
    let sum_y: f64 = valid.iter().map(|(_, y)| y).sum();
    let sum_xy: f64 = valid.iter().map(|(x, y)| x * y).sum();
    let sum_xx: f64 = valid.iter().map(|(x, _)| x * x).sum();
    let denom = n * sum_xx - sum_x * sum_x;
    if denom.abs() < f64::EPSILON {
        0.0
    } else {
        (n * sum_xy - sum_x * sum_y) / denom
    }
}

fn compute_velocity(snapshot_results: &[SnapshotResult]) -> Velocity {
    if snapshot_results.len() < 2 {
        return Velocity {
            violations_resolved_avg: 0.0,
            violations_introduced_avg: 0.0,
            net_improvement_rate: 0.0,
        };
    }

    let mut total_resolved = 0i64;
    let mut total_introduced = 0i64;
    let mut transitions = 0;

    for window in snapshot_results.windows(2) {
        let prev_violations: std::collections::HashSet<(String, String)> = window[0]
            .2
            .violations
            .iter()
            .map(|v| (v.from_module.clone(), v.to_module.clone()))
            .collect();
        let curr_violations: std::collections::HashSet<(String, String)> = window[1]
            .2
            .violations
            .iter()
            .map(|v| (v.from_module.clone(), v.to_module.clone()))
            .collect();

        let resolved = prev_violations.difference(&curr_violations).count() as i64;
        let introduced = curr_violations.difference(&prev_violations).count() as i64;
        total_resolved += resolved;
        total_introduced += introduced;
        transitions += 1;
    }

    let n = transitions.max(1) as f64;
    Velocity {
        violations_resolved_avg: total_resolved as f64 / n,
        violations_introduced_avg: total_introduced as f64 / n,
        net_improvement_rate: (total_resolved - total_introduced) as f64 / n,
    }
}

fn compute_risk_concentration(snapshot_results: &[SnapshotResult]) -> Vec<RiskModule> {
    let last = match snapshot_results.last() {
        Some(s) => s,
        None => return Vec::new(),
    };
    let mut top: Vec<&crate::analyzer::ModuleMetrics> = last
        .2
        .hotspots
        .iter()
        .filter(|h| h.blast_radius > 0)
        .collect();
    top.sort_by_key(|h| std::cmp::Reverse(h.blast_radius));
    let top: Vec<&crate::analyzer::ModuleMetrics> = top.into_iter().take(5).collect();

    let prev_blast: BTreeMap<String, usize> = if snapshot_results.len() >= 2 {
        let prev = &snapshot_results[snapshot_results.len() - 2];
        prev.2
            .hotspots
            .iter()
            .map(|h| (h.path.clone(), h.blast_radius))
            .collect()
    } else {
        BTreeMap::new()
    };

    top.into_iter()
        .map(|h| {
            let prev = prev_blast.get(&h.path).copied();
            let direction = match prev {
                Some(p) if h.blast_radius > p => "rising".to_string(),
                Some(p) if h.blast_radius < p => "falling".to_string(),
                Some(_) => "stable".to_string(),
                None => "new".to_string(),
            };
            RiskModule {
                path: short_path(&h.path),
                current_blast_radius: h.blast_radius,
                previous_blast_radius: prev,
                direction,
            }
        })
        .collect()
}

fn short_path(path: &str) -> String {
    std::path::Path::new(path)
        .file_name()
        .and_then(|f| f.to_str())
        .unwrap_or(path)
        .to_string()
}

fn build_headline(points: &[SnapshotPoint]) -> String {
    if points.len() < 2 {
        return format!(
            "Tracking architecture health (current score: {:.1}/100)",
            points.last().map(|p| p.score).unwrap_or(0.0)
        );
    }
    let first = points.first().unwrap().score;
    let last = points.last().unwrap().score;
    let delta = last - first;
    let trend_word = if delta.abs() < 0.5 {
        "stable"
    } else if delta > 0.0 {
        "improving"
    } else {
        "declining"
    };
    let n = points.len();
    format!(
        "Architecture health is {} ({:+.1} points over the last {} snapshot{})",
        trend_word,
        delta,
        n,
        if n == 1 { "" } else { "s" }
    )
}
