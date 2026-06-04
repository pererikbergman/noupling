//! Per-directory abstractness: ratio of abstract type declarations
//! (traits, interfaces, abstract classes) to all type declarations.
//!
//! Martin's `A = abstract_count / (abstract_count + concrete_count)`. Range
//! 0.0 (all concrete) to 1.0 (all abstract). Combined with the instability
//! metric, gives Distance from Main Sequence.

use fxhash::FxHashMap;

use crate::core::Module;
use crate::core::ModuleTypeCounts;

/// Abstractness metric for a directory.
#[derive(Debug, Clone)]
pub struct AbstractnessMetric {
    pub dir: String,
    pub abstract_count: usize,
    pub concrete_count: usize,
    /// Abstractness A = abstract_count / (abstract_count + concrete_count).
    pub abstractness: f64,
}

/// Compute per-directory abstractness. Returns only directories that contain
/// at least one type declaration, sorted by directory path for stable output.
pub fn compute_abstractness(
    modules: &[Module],
    type_counts: &[ModuleTypeCounts],
) -> Vec<AbstractnessMetric> {
    let path_to_counts: FxHashMap<&str, (usize, usize)> = type_counts
        .iter()
        .map(|t| {
            (
                t.module_path.as_str(),
                (t.counts.abstract_count, t.counts.concrete_count),
            )
        })
        .collect();

    let mut per_dir: FxHashMap<String, (usize, usize)> = FxHashMap::default();
    for module in modules {
        let Some((abstr, conc)) = path_to_counts.get(module.path.as_str()).copied() else {
            continue;
        };
        let dir = std::path::Path::new(&module.path)
            .parent()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default();
        if dir.is_empty() {
            continue;
        }
        let entry = per_dir.entry(dir).or_default();
        entry.0 += abstr;
        entry.1 += conc;
    }

    let mut result: Vec<AbstractnessMetric> = per_dir
        .into_iter()
        .filter(|(_, (a, c))| a + c > 0)
        .map(|(dir, (a, c))| {
            let total = (a + c) as f64;
            AbstractnessMetric {
                dir,
                abstract_count: a,
                concrete_count: c,
                abstractness: a as f64 / total,
            }
        })
        .collect();
    result.sort_by(|a, b| a.dir.cmp(&b.dir));
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::TypeCounts;
    use crate::core::{Module, ModuleType};

    fn file_module(path: &str) -> Module {
        Module {
            id: path.to_string(),
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

    #[test]
    fn aggregates_single_module_into_parent_dir() {
        let modules = vec![file_module("src/lib.rs")];
        let counts = vec![ModuleTypeCounts {
            module_path: "src/lib.rs".into(),
            counts: TypeCounts {
                abstract_count: 1,
                concrete_count: 2,
            },
        }];
        let result = compute_abstractness(&modules, &counts);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].dir, "src");
        assert_eq!(result[0].abstract_count, 1);
        assert_eq!(result[0].concrete_count, 2);
        assert!((result[0].abstractness - 1.0 / 3.0).abs() < 1e-9);
    }
}
