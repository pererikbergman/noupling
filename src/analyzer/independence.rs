//! Per top-level-module independence: ratio of internal dependencies to
//! total (internal + external) dependencies.

use fxhash::{FxHashMap, FxHashSet};

use crate::core::{Dependency, Module};

/// Independence score for a top-level module/directory.
#[derive(Debug, Clone)]
pub struct ModuleIndependence {
    /// Directory path.
    pub dir: String,
    /// Number of files in this module.
    pub file_count: usize,
    /// Dependencies where both source and target are within this module.
    pub internal_deps: usize,
    /// Dependencies where source is in this module but target is outside.
    pub external_deps: usize,
    /// Independence score: internal / (internal + external). Range 0.0 to 1.0.
    pub independence: f64,
}

/// Compute independence for each top-level directory containing at least 2 files.
/// Returned sorted by independence ascending (least-independent modules first).
pub fn compute_independence(
    modules: &[Module],
    dependencies: &[Dependency],
) -> Vec<ModuleIndependence> {
    let mut top_dirs: FxHashMap<String, FxHashSet<&str>> = FxHashMap::default();
    for module in modules {
        // Get the first path component as the top-level directory
        let top = module
            .path
            .split('/')
            .next()
            .unwrap_or(&module.path)
            .to_string();
        // Only group if there's depth (file is not at root)
        if module.path.contains('/') {
            top_dirs.entry(top).or_default().insert(module.id.as_str());
        }
    }

    let mut independence: Vec<ModuleIndependence> = top_dirs
        .iter()
        .filter(|(_, files)| files.len() >= 2)
        .map(|(dir, files)| {
            let internal = dependencies
                .iter()
                .filter(|d| {
                    files.contains(d.from_module_id.as_str())
                        && files.contains(d.to_module_id.as_str())
                })
                .count();
            let external = dependencies
                .iter()
                .filter(|d| {
                    files.contains(d.from_module_id.as_str())
                        && !files.contains(d.to_module_id.as_str())
                })
                .count();
            let total = internal + external;
            let score = if total > 0 {
                internal as f64 / total as f64
            } else {
                1.0
            };
            ModuleIndependence {
                dir: dir.clone(),
                file_count: files.len(),
                internal_deps: internal,
                external_deps: external,
                independence: score,
            }
        })
        .collect();
    independence.sort_by(|a, b| a.independence.partial_cmp(&b.independence).unwrap());
    independence
}
