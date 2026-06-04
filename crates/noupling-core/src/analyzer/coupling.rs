//! Sibling-coupling detection: D_acc accumulation, directory tree, and the
//! BFS sweep that walks every parent directory looking for cross-sibling
//! imports (and the circular dependencies among them).

use std::collections::BTreeMap;

use fxhash::{FxHashMap, FxHashSet};

use super::cycles::detect_sibling_cycles;
use super::DependencyDirection;
use crate::core::{Dependency, Module};

/// A detected coupling violation or circular dependency between modules.
#[derive(Debug, Clone)]
pub struct CouplingViolation {
    /// First directory involved in the violation.
    pub dir_a: String,
    /// Second directory involved in the violation.
    pub dir_b: String,
    /// Source file containing the problematic import.
    pub from_module: String,
    /// Target file being imported.
    pub to_module: String,
    /// Line number of the import in the source file.
    pub line_number: i32,
    /// Directory depth where the violation occurs.
    pub depth: i32,
    /// Number of import statements between this directory pair.
    pub weight: usize,
    /// Severity score. Coupling: `weight/(depth+1)`. Circular: `modules/(depth+1)/10`.
    pub severity: f64,
    /// Architectural direction of this dependency (sibling, circular, etc.).
    pub direction: DependencyDirection,
    /// Relationship Risk Index = direction_weight × density.
    /// Computed by `apply_risk_weights()`. Zero until weights are applied.
    pub rri: f64,
    /// Whether this is a circular dependency (vs. a coupling violation).
    pub is_circular: bool,
    /// For circular deps: the full cycle path as directory paths.
    pub cycle_path: Vec<String>,
    /// For circular deps: `(from_file, to_file, line_number)` for each hop in the cycle.
    pub cycle_hop_files: Vec<(String, String, i32)>,
    /// For circular deps: number of nodes in the cycle (2 = mutual, 3 = triangle, etc.).
    pub cycle_order: usize,
    /// For circular deps: number of imports crossing each hop in the cycle.
    pub cycle_hop_counts: Vec<usize>,
    /// For circular deps: the weakest hop to break (fewest imports). Format: "dir_a -> dir_b (N imports)".
    pub weakest_link: Option<String>,
    /// For circular deps: number of imports to remove at the weakest link to break the cycle.
    pub break_cost: usize,
}

/// Derive virtual directory tree from module file paths.
/// Returns a map of directory path -> list of child directory paths.
pub(super) fn build_dir_tree(modules: &[Module]) -> BTreeMap<String, Vec<String>> {
    let mut all_dirs: FxHashSet<String> = FxHashSet::default();

    // Collect all directory paths from module paths
    for module in modules {
        let path = std::path::Path::new(&module.path);
        let mut current = path.parent();
        while let Some(dir) = current {
            let dir_str = dir.to_string_lossy().to_string();
            if dir_str.is_empty() {
                break;
            }
            all_dirs.insert(dir_str);
            current = dir.parent();
        }
    }

    // Build parent -> children mapping
    let mut tree: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for dir in &all_dirs {
        let parent = std::path::Path::new(dir)
            .parent()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default();

        tree.entry(parent).or_default().push(dir.clone());
    }

    // Sort children for deterministic output
    for children in tree.values_mut() {
        children.sort();
    }

    tree
}

/// Get the depth of a directory path (number of components).
pub(super) fn dir_depth(dir: &str) -> i32 {
    if dir.is_empty() {
        return -1;
    }
    std::path::Path::new(dir).components().count() as i32
}

/// Compute D_acc for every directory: the set of external dependency target module IDs.
/// "External" means the target module is NOT under the same directory.
pub(super) fn compute_dacc(
    modules: &[Module],
    dependencies: &[Dependency],
) -> FxHashMap<String, FxHashSet<String>> {
    // Map module_id -> path for quick lookup
    let id_to_path: FxHashMap<&str, &str> = modules
        .iter()
        .map(|m| (m.id.as_str(), m.path.as_str()))
        .collect();

    // Map module_id -> parent dir
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

    // Collect all directory paths
    let mut all_dirs: FxHashSet<String> = FxHashSet::default();
    for module in modules {
        let path = std::path::Path::new(&module.path);
        let mut current = path.parent();
        while let Some(dir) = current {
            let dir_str = dir.to_string_lossy().to_string();
            if dir_str.is_empty() {
                break;
            }
            all_dirs.insert(dir_str);
            current = dir.parent();
        }
    }

    // For each directory, collect all dependencies where:
    // - the source module is under this directory (including sub-trees)
    // - the target module is NOT under this directory (external dep)
    let mut dacc: FxHashMap<String, FxHashSet<String>> = FxHashMap::default();

    // Sort dirs by depth descending so we process leaves first
    let mut sorted_dirs: Vec<&String> = all_dirs.iter().collect();
    sorted_dirs.sort_by_key(|a| std::cmp::Reverse(dir_depth(a)));

    for dir in &sorted_dirs {
        let mut external_deps: FxHashSet<String> = FxHashSet::default();
        let dir_prefix = format!("{}/", dir);

        // Direct file dependencies
        for dep in dependencies {
            let from_path = match id_to_path.get(dep.from_module_id.as_str()) {
                Some(p) => *p,
                None => continue,
            };
            let to_path = match id_to_path.get(dep.to_module_id.as_str()) {
                Some(p) => *p,
                None => continue,
            };

            // Source must be under this directory
            let from_under = from_path.starts_with(&dir_prefix)
                || id_to_dir
                    .get(dep.from_module_id.as_str())
                    .map(|d| d.as_str() == dir.as_str())
                    .unwrap_or(false);
            if !from_under {
                continue;
            }

            // Target must NOT be under this directory (external dep)
            let to_under = to_path.starts_with(&dir_prefix)
                || id_to_dir
                    .get(dep.to_module_id.as_str())
                    .map(|d| d.as_str() == dir.as_str())
                    .unwrap_or(false);
            if to_under {
                continue;
            }

            external_deps.insert(dep.to_module_id.clone());
        }

        // Propagate child D_acc upward: merge children's external deps
        // (only those that are also external to this directory)
        for child_dir in all_dirs.iter() {
            if child_dir.starts_with(&dir_prefix) && child_dir != *dir {
                if let Some(child_dacc) = dacc.get(child_dir) {
                    for target_id in child_dacc {
                        // Check if target is still external to this directory
                        if let Some(to_path) = id_to_path.get(target_id.as_str()) {
                            let to_under = to_path.starts_with(&dir_prefix)
                                || id_to_dir
                                    .get(target_id.as_str())
                                    .map(|d| d.as_str() == dir.as_str())
                                    .unwrap_or(false);
                            if !to_under {
                                external_deps.insert(target_id.clone());
                            }
                        }
                    }
                }
            }
        }

        dacc.insert(dir.to_string(), external_deps);
    }

    dacc
}

/// Walk every parent directory, detect circular dependencies among siblings,
/// and emit one `CouplingViolation` per cross-sibling import. Coupling
/// violations are deduplicated by `(dir_a, dir_b, depth)` and weighted by the
/// number of imports between the pair; circular violations carry the full
/// cycle metadata (path, hop files, hop import counts, weakest link, break
/// cost) computed by `cycles::detect_sibling_cycles`. The returned list is
/// sorted by severity descending.
pub(super) fn compute_coupling_violations(
    modules: &[Module],
    dependencies: &[Dependency],
) -> Vec<CouplingViolation> {
    let dir_tree = build_dir_tree(modules);
    let dacc = compute_dacc(modules, dependencies);

    // Map module_id -> dir for checking which dir a target belongs to
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

    let id_to_path: FxHashMap<&str, &str> = modules
        .iter()
        .map(|m| (m.id.as_str(), m.path.as_str()))
        .collect();

    let mut violations = Vec::new();

    // BFS: for each parent directory, check sibling pairs for coupling AND cycles
    for (parent, children) in &dir_tree {
        let depth = if parent.is_empty() {
            0
        } else {
            dir_depth(parent) + 1
        };

        // Detect circular dependencies among siblings at this level using D_acc
        let sibling_cycles =
            detect_sibling_cycles(children, &dacc, &id_to_dir, &id_to_path, dependencies);
        for cycle in sibling_cycles {
            if cycle.dir_path.len() < 2 {
                continue;
            }
            let first_dir = &cycle.dir_path[0];
            let last_target = &cycle.dir_path[cycle.dir_path.len() - 2];

            // Find weakest link: hop with fewest imports
            let (weakest_link, break_cost) = if !cycle.hop_import_counts.is_empty() {
                let min_idx = cycle
                    .hop_import_counts
                    .iter()
                    .enumerate()
                    .min_by_key(|(_, &count)| count)
                    .map(|(idx, _)| idx)
                    .unwrap_or(0);
                let from_dir = &cycle.dir_path[min_idx];
                let to_dir = &cycle.dir_path[min_idx + 1];
                let count = cycle.hop_import_counts[min_idx];
                (
                    Some(format!(
                        "{} -> {} ({} import{})",
                        from_dir,
                        to_dir,
                        count,
                        if count == 1 { "" } else { "s" }
                    )),
                    count,
                )
            } else {
                (None, 0)
            };

            violations.push(CouplingViolation {
                dir_a: first_dir.clone(),
                dir_b: last_target.clone(),
                from_module: first_dir.clone(),
                to_module: last_target.clone(),
                line_number: 0,
                depth,
                weight: 0,
                severity: modules.len() as f64 / (depth as f64 + 1.0) / 10.0,
                direction: DependencyDirection::Circular,
                rri: 0.0,
                is_circular: true,
                cycle_path: cycle.dir_path.clone(),
                cycle_hop_files: cycle.hop_files.clone(),
                cycle_order: cycle.dir_path.len() - 1, // -1 because last entry closes the cycle
                cycle_hop_counts: cycle.hop_import_counts.clone(),
                weakest_link,
                break_cost,
            });
        }

        for i in 0..children.len() {
            for j in (i + 1)..children.len() {
                let dir_a = &children[i];
                let dir_b = &children[j];

                let dacc_a = dacc.get(dir_a);
                let dacc_b = dacc.get(dir_b);

                // Check if D_acc(A) references modules under B
                if let Some(deps_a) = dacc_a {
                    for target_id in deps_a {
                        if let Some(target_dir) = id_to_dir.get(target_id.as_str()) {
                            let b_prefix = format!("{}/", dir_b);
                            if target_dir == dir_b || target_dir.starts_with(&b_prefix) {
                                // Find the source module(s) that caused this
                                for dep in dependencies {
                                    if &dep.to_module_id == target_id {
                                        let from_dir = id_to_dir.get(dep.from_module_id.as_str());
                                        let a_prefix = format!("{}/", dir_a);
                                        if let Some(fd) = from_dir {
                                            if fd == dir_a || fd.starts_with(&a_prefix) {
                                                violations.push(CouplingViolation {
                                                    dir_a: dir_a.clone(),
                                                    dir_b: dir_b.clone(),
                                                    from_module: id_to_path
                                                        .get(dep.from_module_id.as_str())
                                                        .unwrap_or(&"")
                                                        .to_string(),
                                                    to_module: id_to_path
                                                        .get(dep.to_module_id.as_str())
                                                        .unwrap_or(&"")
                                                        .to_string(),
                                                    line_number: dep.line_number,
                                                    depth,
                                                    weight: 1,
                                                    severity: 1.0 / (depth as f64 + 1.0),
                                                    direction: DependencyDirection::Sibling,
                                                    rri: 0.0,
                                                    is_circular: false,
                                                    cycle_path: Vec::new(),
                                                    cycle_hop_files: Vec::new(),
                                                    cycle_order: 0,
                                                    cycle_hop_counts: Vec::new(),
                                                    weakest_link: None,
                                                    break_cost: 0,
                                                });
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                // Check if D_acc(B) references modules under A
                if let Some(deps_b) = dacc_b {
                    for target_id in deps_b {
                        if let Some(target_dir) = id_to_dir.get(target_id.as_str()) {
                            let a_prefix = format!("{}/", dir_a);
                            if target_dir == dir_a || target_dir.starts_with(&a_prefix) {
                                for dep in dependencies {
                                    if &dep.to_module_id == target_id {
                                        let from_dir = id_to_dir.get(dep.from_module_id.as_str());
                                        let b_prefix = format!("{}/", dir_b);
                                        if let Some(fd) = from_dir {
                                            if fd == dir_b || fd.starts_with(&b_prefix) {
                                                violations.push(CouplingViolation {
                                                    dir_a: dir_b.clone(),
                                                    dir_b: dir_a.clone(),
                                                    from_module: id_to_path
                                                        .get(dep.from_module_id.as_str())
                                                        .unwrap_or(&"")
                                                        .to_string(),
                                                    to_module: id_to_path
                                                        .get(dep.to_module_id.as_str())
                                                        .unwrap_or(&"")
                                                        .to_string(),
                                                    line_number: dep.line_number,
                                                    depth,
                                                    weight: 1,
                                                    severity: 1.0 / (depth as f64 + 1.0),
                                                    direction: DependencyDirection::Sibling,
                                                    rri: 0.0,
                                                    is_circular: false,
                                                    cycle_path: Vec::new(),
                                                    cycle_hop_files: Vec::new(),
                                                    cycle_order: 0,
                                                    cycle_hop_counts: Vec::new(),
                                                    weakest_link: None,
                                                    break_cost: 0,
                                                });
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // Aggregate coupling violations by (dir_a, dir_b, depth) and count weight
    let mut weight_map: FxHashMap<(String, String, i32), usize> = FxHashMap::default();
    for v in &violations {
        if !v.is_circular {
            let key = (v.dir_a.clone(), v.dir_b.clone(), v.depth);
            *weight_map.entry(key).or_insert(0) += 1;
        }
    }

    // Dedup coupling violations, keeping one per (dir_a, dir_b, depth) with weight
    let mut seen: FxHashSet<(String, String, i32)> = FxHashSet::default();
    violations.retain_mut(|v| {
        if v.is_circular {
            return true;
        }
        let key = (v.dir_a.clone(), v.dir_b.clone(), v.depth);
        if seen.contains(&key) {
            return false;
        }
        seen.insert(key.clone());
        let w = *weight_map.get(&key).unwrap_or(&1);
        v.weight = w;
        v.severity = w as f64 / (v.depth as f64 + 1.0);
        true
    });

    // Sort by severity descending
    violations.sort_by(|a, b| b.severity.partial_cmp(&a.severity).unwrap());

    violations
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::ModuleType;

    fn make_module(id: &str, path: &str) -> Module {
        Module {
            id: id.to_string(),
            snapshot_id: "snap".to_string(),
            parent_id: None,
            name: std::path::Path::new(path)
                .file_name()
                .unwrap()
                .to_string_lossy()
                .to_string(),
            path: path.to_string(),
            module_type: ModuleType::File,
            depth: std::path::Path::new(path).components().count() as i32,
        }
    }

    fn make_dep(from: &str, to: &str, line: i32) -> Dependency {
        Dependency {
            from_module_id: from.to_string(),
            to_module_id: to.to_string(),
            line_number: line,
        }
    }

    // ── D_acc tests ──

    #[test]
    fn dacc_excludes_internal_deps() {
        // Two files in the same directory depending on each other = internal
        let modules = vec![
            make_module("a", "src/scanner/parser.rs"),
            make_module("b", "src/scanner/resolver.rs"),
        ];
        let deps = vec![make_dep("a", "b", 1)];
        let dacc = compute_dacc(&modules, &deps);
        // src/scanner's D_acc should be empty (internal dep)
        let scanner_dacc = dacc.get("src/scanner").unwrap();
        assert!(scanner_dacc.is_empty());
    }

    #[test]
    fn dacc_includes_external_deps() {
        let modules = vec![
            make_module("a", "src/scanner/mod.rs"),
            make_module("b", "src/core/mod.rs"),
        ];
        let deps = vec![make_dep("a", "b", 5)];
        let dacc = compute_dacc(&modules, &deps);
        let scanner_dacc = dacc.get("src/scanner").unwrap();
        assert!(scanner_dacc.contains("b"));
    }

    #[test]
    fn dacc_aggregates_subtree() {
        // scanner/parser.rs depends on core/mod.rs
        // scanner dir should accumulate this in D_acc (external to scanner)
        // But src should NOT (both scanner and core are under src, so it's internal to src)
        let modules = vec![
            make_module("a", "src/scanner/parser.rs"),
            make_module("b", "src/core/mod.rs"),
        ];
        let deps = vec![make_dep("a", "b", 1)];
        let dacc = compute_dacc(&modules, &deps);

        // src/scanner should have external dep to b
        assert!(dacc.get("src/scanner").unwrap().contains("b"));
        // src should NOT have it (both are under src, so internal)
        assert!(dacc.get("src").unwrap().is_empty());
    }

    #[test]
    fn dacc_propagates_to_parent_when_truly_external() {
        // scanner depends on something outside src entirely
        let modules = vec![
            make_module("a", "src/scanner/parser.rs"),
            make_module("b", "lib/utils.rs"),
        ];
        let deps = vec![make_dep("a", "b", 1)];
        let dacc = compute_dacc(&modules, &deps);

        // src/scanner has external dep
        assert!(dacc.get("src/scanner").unwrap().contains("b"));
        // src also has it (b is outside src)
        assert!(dacc.get("src").unwrap().contains("b"));
    }
}
