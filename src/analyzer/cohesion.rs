//! Per-directory cohesion metrics: ratio of internal dependencies to the
//! maximum possible internal dependencies among files in the directory.

use fxhash::{FxHashMap, FxHashSet};

use crate::core::{Dependency, Module};

/// Whether a directory is a leaf-ish package (has direct files) or a container
/// that exists only to group subdirectories. See `docs/dependency-graph.md`
/// § Analysis Step 2 for the architectural significance of the split.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DirectoryKind {
    /// Directory with no direct files; only subdirectories below it. Pure
    /// grouping folder (e.g. `src/features/`). Cohesion is undefined for these.
    Container,
    /// Directory with at least one direct file. Cohesion is defined.
    Package,
}

/// Cohesion metrics for a directory.
#[derive(Debug, Clone)]
pub struct CohesionMetrics {
    /// Directory path.
    pub dir: String,
    /// Whether the directory is a Container or a Package.
    pub kind: DirectoryKind,
    /// Count of "logical-node" children. For Containers this is the count of
    /// direct subdirectories. For Packages today this is the count of direct
    /// files; #225 will widen this to also include direct subdirectories.
    pub n_children: usize,
    /// Count of edges crossing between logical-node children of this directory.
    pub internal_deps: usize,
    /// Cohesion score: `internal_deps / (n_children * (n_children - 1))`.
    /// `None` for Containers (undefined). `Some(value)` for Packages.
    pub cohesion: Option<f64>,
}

/// Compute per-directory cohesion for every directory in the project.
///
/// Both Containers and Packages appear in the result. Output is sorted with
/// Packages first (by cohesion ascending), then Containers (alphabetically by
/// dir). #225 will widen the algorithm to treat subdirectories as opaque
/// logical nodes per `docs/dependency-graph.md` § Analysis Step 2; this
/// implementation preserves the prior file ↔ file algorithm for Packages and
/// only adds the Container classification + `Option<f64>` shape.
pub fn compute_cohesion(modules: &[Module], dependencies: &[Dependency]) -> Vec<CohesionMetrics> {
    // Files directly under each directory.
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

    // All directories in the project — ancestors of every file path.
    // Containers will appear here but not in `dir_files`.
    let mut all_dirs: FxHashSet<String> = FxHashSet::default();
    for module in modules {
        let mut current = std::path::Path::new(&module.path).parent();
        while let Some(dir) = current {
            let dir_str = dir.to_string_lossy().to_string();
            if dir_str.is_empty() {
                break;
            }
            all_dirs.insert(dir_str);
            current = dir.parent();
        }
    }

    let mut cohesion: Vec<CohesionMetrics> = all_dirs
        .iter()
        .map(|dir| {
            match dir_files.get(dir) {
                // Package: at least one direct file.
                Some(files) => {
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
                        kind: DirectoryKind::Package,
                        n_children: n,
                        internal_deps,
                        cohesion: Some(cohesion_score),
                    }
                }
                // Container: no direct files, only subdirectories.
                None => CohesionMetrics {
                    dir: dir.clone(),
                    kind: DirectoryKind::Container,
                    n_children: 0, // #225 will set this to the subdir count.
                    internal_deps: 0,
                    cohesion: None,
                },
            }
        })
        .collect();

    // Packages first (ordered by cohesion ascending), then Containers (alpha).
    cohesion.sort_by(|a, b| match (a.kind, b.kind) {
        (DirectoryKind::Package, DirectoryKind::Container) => std::cmp::Ordering::Less,
        (DirectoryKind::Container, DirectoryKind::Package) => std::cmp::Ordering::Greater,
        (DirectoryKind::Package, DirectoryKind::Package) => a
            .cohesion
            .unwrap_or(0.0)
            .partial_cmp(&b.cohesion.unwrap_or(0.0))
            .unwrap_or(std::cmp::Ordering::Equal),
        (DirectoryKind::Container, DirectoryKind::Container) => a.dir.cmp(&b.dir),
    });
    cohesion
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{Module, ModuleType};

    fn file_module(id: &str, path: &str) -> Module {
        Module {
            id: id.into(),
            snapshot_id: "snap".into(),
            parent_id: None,
            name: std::path::Path::new(path)
                .file_name()
                .unwrap()
                .to_string_lossy()
                .into_owned(),
            path: path.into(),
            module_type: ModuleType::File,
            depth: 1,
        }
    }

    fn dep(from: &str, to: &str) -> Dependency {
        Dependency {
            from_module_id: from.into(),
            to_module_id: to.into(),
            line_number: 1,
        }
    }

    #[test]
    fn package_with_two_files_and_one_internal_dep_computes_cohesion() {
        // src/scanner/ has two direct files; one imports the other.
        // n_children = 2 → pairs = 2 → cohesion = 1/2 = 0.5
        let modules = vec![
            file_module("a", "src/scanner/mod.rs"),
            file_module("b", "src/scanner/discovery.rs"),
        ];
        let deps = vec![dep("a", "b")];

        let result = compute_cohesion(&modules, &deps);

        let scanner = result
            .iter()
            .find(|c| c.dir == "src/scanner")
            .expect("src/scanner should appear");
        assert_eq!(scanner.kind, DirectoryKind::Package);
        assert_eq!(scanner.n_children, 2);
        assert_eq!(scanner.internal_deps, 1);
        assert_eq!(scanner.cohesion, Some(0.5));
    }

    #[test]
    fn single_file_package_returns_some_zero_not_division_by_zero() {
        // src/utils/ has exactly one direct file. n=1, pairs=0.
        // Cohesion must be Some(0.0), not None and not NaN.
        let modules = vec![file_module("a", "src/utils/lone.rs")];
        let deps = vec![];

        let result = compute_cohesion(&modules, &deps);

        let utils = result
            .iter()
            .find(|c| c.dir == "src/utils")
            .expect("src/utils should appear");
        assert_eq!(utils.kind, DirectoryKind::Package);
        assert_eq!(utils.n_children, 1);
        assert_eq!(utils.cohesion, Some(0.0));
    }

    #[test]
    fn container_dir_has_kind_container_and_no_cohesion() {
        // src/features/ holds two subdirs (auth, billing); no direct files.
        // → Container, cohesion: None, present in output.
        let modules = vec![
            file_module("a", "src/features/auth/login.rs"),
            file_module("b", "src/features/billing/invoice.rs"),
        ];
        let deps = vec![];

        let result = compute_cohesion(&modules, &deps);

        let features = result
            .iter()
            .find(|c| c.dir == "src/features")
            .expect("src/features should appear in cohesion output");
        assert_eq!(features.kind, DirectoryKind::Container);
        assert_eq!(features.cohesion, None);
    }
}
