//! Per-directory instability: Martin's `I = Ce / (Ca + Ce)`.
//!
//! - `Ca` (afferent couplings): incoming edges — modules outside the directory
//!   that import modules inside it.
//! - `Ce` (efferent couplings): outgoing edges — modules inside the directory
//!   that import modules outside it.
//! - `I` ranges 0.0 (fully stable, depended on but doesn't depend) to 1.0
//!   (fully unstable, depends on but isn't depended on).
//!
//! Internal edges (both endpoints inside the same directory) are excluded — they
//! don't contribute to either the directory's stability or its instability.
//!
//! Per-file instability lives in `analyzer::metrics::ModuleMetrics`. This module
//! deliberately operates at the directory level because Martin's metric is
//! conventionally a package-level concept, and it composes with abstractness
//! (`A = abstract / (abstract + concrete)`) to give the Distance from Main
//! Sequence `D = |A + I - 1|`.

use fxhash::{FxHashMap, FxHashSet};

use crate::core::{Dependency, Module};

/// Per-directory afferent/efferent counts and the derived instability.
#[derive(Debug, Clone)]
pub struct InstabilityMetric {
    pub dir: String,
    /// Incoming couplings: edges from outside this directory that point into it.
    pub ca: usize,
    /// Outgoing couplings: edges from inside this directory that point outside.
    pub ce: usize,
    /// Martin's instability: `Ce / (Ca + Ce)`.
    pub instability: f64,
}

/// A violation of the Stable Dependencies Principle: a more-stable directory
/// (lower I) depends on a less-stable directory (higher I).
#[derive(Debug, Clone)]
pub struct StabilityViolation {
    pub from_dir: String,
    pub to_dir: String,
    pub from_instability: f64,
    pub to_instability: f64,
}

/// Compute per-directory instability. Returns only directories that have at
/// least one external edge (Ca + Ce > 0), sorted by directory path for stable
/// output.
pub fn compute_directory_instability(
    modules: &[Module],
    dependencies: &[Dependency],
) -> Vec<InstabilityMetric> {
    let id_to_dir: FxHashMap<&str, String> = modules
        .iter()
        .map(|m| {
            let dir = std::path::Path::new(&m.path)
                .parent()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_default();
            (m.id.as_str(), dir)
        })
        .collect();

    let mut all_dirs: FxHashSet<String> = FxHashSet::default();
    for d in id_to_dir.values() {
        if !d.is_empty() {
            all_dirs.insert(d.clone());
        }
    }

    let mut per_dir: FxHashMap<String, (usize, usize)> = FxHashMap::default();
    for d in &all_dirs {
        per_dir.insert(d.clone(), (0, 0));
    }

    for dep in dependencies {
        let from_dir = match id_to_dir.get(dep.from_module_id.as_str()) {
            Some(d) if !d.is_empty() => d.clone(),
            _ => continue,
        };
        let to_dir = match id_to_dir.get(dep.to_module_id.as_str()) {
            Some(d) if !d.is_empty() => d.clone(),
            _ => continue,
        };
        if from_dir == to_dir {
            continue;
        }
        if let Some(e) = per_dir.get_mut(&from_dir) {
            e.1 += 1; // ce
        }
        if let Some(e) = per_dir.get_mut(&to_dir) {
            e.0 += 1; // ca
        }
    }

    let mut result: Vec<InstabilityMetric> = per_dir
        .into_iter()
        .filter(|(_, (ca, ce))| ca + ce > 0)
        .map(|(dir, (ca, ce))| {
            let total = (ca + ce) as f64;
            InstabilityMetric {
                dir,
                ca,
                ce,
                instability: ce as f64 / total,
            }
        })
        .collect();
    result.sort_by(|a, b| a.dir.cmp(&b.dir));
    result
}

/// Detect Stable Dependencies Principle violations: cross-directory edges
/// `from → to` where `I(from) < I(to)`. The more-stable directory shouldn't
/// depend on the less-stable one; stability should flow inward, not outward.
///
/// Returns one entry per violating directory pair (deduplicated even if many
/// individual file-level imports realize the same pair).
pub fn compute_stability_violations(
    modules: &[Module],
    dependencies: &[Dependency],
    instability: &[InstabilityMetric],
) -> Vec<StabilityViolation> {
    let id_to_dir: FxHashMap<&str, String> = modules
        .iter()
        .map(|m| {
            let dir = std::path::Path::new(&m.path)
                .parent()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_default();
            (m.id.as_str(), dir)
        })
        .collect();
    let dir_to_i: FxHashMap<&str, f64> = instability
        .iter()
        .map(|m| (m.dir.as_str(), m.instability))
        .collect();

    let mut seen: FxHashSet<(String, String)> = FxHashSet::default();
    let mut result = Vec::new();
    for dep in dependencies {
        let from_dir = match id_to_dir.get(dep.from_module_id.as_str()) {
            Some(d) if !d.is_empty() => d.clone(),
            _ => continue,
        };
        let to_dir = match id_to_dir.get(dep.to_module_id.as_str()) {
            Some(d) if !d.is_empty() => d.clone(),
            _ => continue,
        };
        if from_dir == to_dir {
            continue;
        }
        let from_i = match dir_to_i.get(from_dir.as_str()) {
            Some(i) => *i,
            None => continue,
        };
        let to_i = match dir_to_i.get(to_dir.as_str()) {
            Some(i) => *i,
            None => continue,
        };
        if from_i < to_i && seen.insert((from_dir.clone(), to_dir.clone())) {
            result.push(StabilityViolation {
                from_dir,
                to_dir,
                from_instability: from_i,
                to_instability: to_i,
            });
        }
    }
    result.sort_by(|a, b| a.from_dir.cmp(&b.from_dir).then(a.to_dir.cmp(&b.to_dir)));
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{Dependency, Module, ModuleType};

    fn file_module(id: &str, path: &str) -> Module {
        Module {
            id: id.to_string(),
            snapshot_id: "snap".into(),
            parent_id: None,
            name: std::path::Path::new(path)
                .file_name()
                .unwrap()
                .to_string_lossy()
                .into_owned(),
            path: path.to_string(),
            module_type: ModuleType::File,
            depth: 1,
        }
    }

    fn dep(from: &str, to: &str) -> Dependency {
        Dependency {
            from_module_id: from.to_string(),
            to_module_id: to.to_string(),
            line_number: 1,
        }
    }

    #[test]
    fn flags_sdp_violation_when_stable_dir_depends_on_unstable_dir() {
        // Setup:
        //   stable/  — 5 incoming, 1 outgoing (→ unstable)         → I ≈ 0.17
        //   unstable/ — 1 incoming (← stable), 5 outgoing (→ helpers) → I ≈ 0.83
        //   clients/* import stable; unstable imports helpers/*.
        //
        // The edge `stable → unstable` violates SDP because the more stable
        // module (lower I) depends on the less stable one (higher I).
        let modules = vec![
            file_module("stable", "src/stable/lib.rs"),
            file_module("unstable", "src/unstable/lib.rs"),
            file_module("c1", "src/clients/a.rs"),
            file_module("c2", "src/clients/b.rs"),
            file_module("c3", "src/clients/c.rs"),
            file_module("c4", "src/clients/d.rs"),
            file_module("c5", "src/clients/e.rs"),
            file_module("h1", "src/helpers/a.rs"),
            file_module("h2", "src/helpers/b.rs"),
            file_module("h3", "src/helpers/c.rs"),
            file_module("h4", "src/helpers/d.rs"),
            file_module("h5", "src/helpers/e.rs"),
        ];
        let deps = vec![
            // Clients depend on stable
            dep("c1", "stable"),
            dep("c2", "stable"),
            dep("c3", "stable"),
            dep("c4", "stable"),
            dep("c5", "stable"),
            // The offending edge
            dep("stable", "unstable"),
            // Unstable depends on helpers
            dep("unstable", "h1"),
            dep("unstable", "h2"),
            dep("unstable", "h3"),
            dep("unstable", "h4"),
            dep("unstable", "h5"),
        ];

        let instability = compute_directory_instability(&modules, &deps);
        let stable_i = instability.iter().find(|m| m.dir == "src/stable").unwrap();
        let unstable_i = instability
            .iter()
            .find(|m| m.dir == "src/unstable")
            .unwrap();
        assert!(
            stable_i.instability < unstable_i.instability,
            "stable (I={}) should be more stable than unstable (I={})",
            stable_i.instability,
            unstable_i.instability
        );

        let violations = compute_stability_violations(&modules, &deps, &instability);
        let v = violations
            .iter()
            .find(|v| v.from_dir == "src/stable" && v.to_dir == "src/unstable")
            .expect("expected SDP violation src/stable → src/unstable");
        assert!(v.from_instability < v.to_instability);
    }

    #[test]
    fn computes_ce_for_outgoing_only_module() {
        // app imports core; app has one outgoing edge, zero incoming.
        let modules = vec![
            file_module("app1", "src/app/main.rs"),
            file_module("core1", "src/core/lib.rs"),
        ];
        let deps = vec![dep("app1", "core1")];

        let result = compute_directory_instability(&modules, &deps);

        let app = result.iter().find(|m| m.dir == "src/app").expect("src/app");
        assert_eq!(app.ce, 1);
        assert_eq!(app.ca, 0);
        assert!((app.instability - 1.0).abs() < 1e-9);
    }
}
