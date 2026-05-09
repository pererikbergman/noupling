//! Per-module metrics: fan-in / fan-out, Martin's instability, blast radius.
//! Also covers the per-module external (third-party) import count.

use fxhash::{FxHashMap, FxHashSet};

use crate::core::{Dependency, Module};

/// A module's dependency metrics.
#[derive(Debug, Clone)]
pub struct ModuleMetrics {
    /// Relative path of the module.
    pub path: String,
    /// Number of other modules that import this module (incoming).
    pub fan_in: usize,
    /// Number of modules this module imports (outgoing).
    pub fan_out: usize,
    /// Martin's Instability: fan_out / (fan_in + fan_out). Range 0.0 (stable) to 1.0 (unstable).
    pub instability: f64,
    /// Number of modules transitively affected if this module changes.
    pub blast_radius: usize,
}

/// External dependency count for a single module.
#[derive(Debug, Clone)]
pub struct ExternalDepMetric {
    /// Module file path.
    pub module_path: String,
    /// Number of external (unresolved) imports.
    pub count: usize,
}

/// Compute per-module hotspot metrics: fan-in, fan-out, instability, blast radius.
/// Returned sorted by `fan_in` descending.
pub fn compute_hotspots(modules: &[Module], dependencies: &[Dependency]) -> Vec<ModuleMetrics> {
    let mut fan_in: FxHashMap<&str, usize> = FxHashMap::default();
    let mut fan_out: FxHashMap<&str, usize> = FxHashMap::default();
    let mut reverse_adj: FxHashMap<&str, FxHashSet<&str>> = FxHashMap::default();
    for dep in dependencies {
        *fan_in.entry(dep.to_module_id.as_str()).or_insert(0) += 1;
        *fan_out.entry(dep.from_module_id.as_str()).or_insert(0) += 1;
        reverse_adj
            .entry(dep.to_module_id.as_str())
            .or_default()
            .insert(dep.from_module_id.as_str());
    }

    // BFS on reverse graph for blast radius
    let blast_radius: FxHashMap<&str, usize> = modules
        .iter()
        .map(|m| {
            let mut visited: FxHashSet<&str> = FxHashSet::default();
            let mut queue: std::collections::VecDeque<&str> = std::collections::VecDeque::new();
            queue.push_back(m.id.as_str());
            visited.insert(m.id.as_str());
            while let Some(current) = queue.pop_front() {
                if let Some(dependents) = reverse_adj.get(current) {
                    for &dep in dependents {
                        if visited.insert(dep) {
                            queue.push_back(dep);
                        }
                    }
                }
            }
            (m.id.as_str(), visited.len().saturating_sub(1))
        })
        .collect();

    let mut hotspots: Vec<ModuleMetrics> = modules
        .iter()
        .map(|m| {
            let fi = *fan_in.get(m.id.as_str()).unwrap_or(&0);
            let fo = *fan_out.get(m.id.as_str()).unwrap_or(&0);
            let total = fi + fo;
            ModuleMetrics {
                path: m.path.clone(),
                fan_in: fi,
                fan_out: fo,
                instability: if total > 0 {
                    fo as f64 / total as f64
                } else {
                    0.0
                },
                blast_radius: *blast_radius.get(m.id.as_str()).unwrap_or(&0),
            }
        })
        .collect();
    hotspots.sort_by_key(|h| std::cmp::Reverse(h.fan_in));
    hotspots
}
