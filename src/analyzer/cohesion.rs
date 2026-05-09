//! Per-directory cohesion metrics: ratio of internal dependencies to the
//! maximum possible internal dependencies among files in the directory.

use fxhash::{FxHashMap, FxHashSet};

use crate::core::{Dependency, Module};

/// Cohesion metrics for a directory.
#[derive(Debug, Clone)]
pub struct CohesionMetrics {
    /// Directory path.
    pub dir: String,
    /// Number of files in the directory.
    pub file_count: usize,
    /// Number of internal dependencies (files importing each other within this directory).
    pub internal_deps: usize,
    /// Cohesion score: internal_deps / (file_count * (file_count - 1)). Range 0.0 to 1.0.
    pub cohesion: f64,
}

/// Compute per-directory cohesion. Returns directories with at least 2 files,
/// sorted by cohesion ascending (lowest-cohesion directories first).
pub fn compute_cohesion(modules: &[Module], dependencies: &[Dependency]) -> Vec<CohesionMetrics> {
    let mut dir_files: FxHashMap<String, Vec<&str>> = FxHashMap::default();
    for module in modules {
        let dir = std::path::Path::new(&module.path)
            .parent()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default();
        if !dir.is_empty() {
            dir_files.entry(dir).or_default().push(module.id.as_str());
        }
    }

    let mut cohesion: Vec<CohesionMetrics> = dir_files
        .iter()
        .filter(|(_, files)| files.len() >= 2)
        .map(|(dir, files)| {
            let file_set: FxHashSet<&str> = files.iter().copied().collect();
            let internal_deps = dependencies
                .iter()
                .filter(|d| {
                    file_set.contains(d.from_module_id.as_str())
                        && file_set.contains(d.to_module_id.as_str())
                })
                .count();
            let n = files.len();
            let max_possible = n * (n - 1);
            let cohesion_score = if max_possible > 0 {
                internal_deps as f64 / max_possible as f64
            } else {
                0.0
            };
            CohesionMetrics {
                dir: dir.clone(),
                file_count: n,
                internal_deps,
                cohesion: cohesion_score,
            }
        })
        .collect();
    cohesion.sort_by(|a, b| a.cohesion.partial_cmp(&b.cohesion).unwrap());
    cohesion
}
