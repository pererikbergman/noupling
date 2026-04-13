use fxhash::{FxHashMap, FxHashSet};
use std::collections::BTreeMap;

use crate::core::{Dependency, Module};

#[derive(Debug, Clone)]
pub struct CouplingViolation {
    pub dir_a: String,
    pub dir_b: String,
    pub from_module: String,
    pub to_module: String,
    pub depth: i32,
    pub severity: f64,
    pub is_circular: bool,
}

#[derive(Debug)]
pub struct AuditResult {
    pub violations: Vec<CouplingViolation>,
    pub score: f64,
    pub total_modules: usize,
}

/// Derive virtual directory tree from module file paths.
/// Returns a map of directory path -> list of child directory paths.
fn build_dir_tree(modules: &[Module]) -> BTreeMap<String, Vec<String>> {
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
fn dir_depth(dir: &str) -> i32 {
    if dir.is_empty() {
        return -1;
    }
    std::path::Path::new(dir).components().count() as i32
}

/// Compute D_acc for every directory: the set of external dependency target module IDs.
/// "External" means the target module is NOT under the same directory.
fn compute_dacc(
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
    sorted_dirs.sort_by(|a, b| dir_depth(b).cmp(&dir_depth(a)));

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
            let from_under = from_path.starts_with(&dir_prefix) || id_to_dir.get(dep.from_module_id.as_str()).map(|d| d.as_str() == dir.as_str()).unwrap_or(false);
            if !from_under {
                continue;
            }

            // Target must NOT be under this directory (external dep)
            let to_under = to_path.starts_with(&dir_prefix) || id_to_dir.get(dep.to_module_id.as_str()).map(|d| d.as_str() == dir.as_str()).unwrap_or(false);
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
                            let to_under = to_path.starts_with(&dir_prefix) || id_to_dir.get(target_id.as_str()).map(|d| d.as_str() == dir.as_str()).unwrap_or(false);
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

/// Detect circular dependencies in the module dependency graph.
/// Returns pairs of (from_module_id, to_module_id) that form cycles.
fn detect_cycles(modules: &[Module], dependencies: &[Dependency]) -> Vec<(String, String)> {
    let module_ids: FxHashSet<&str> = modules.iter().map(|m| m.id.as_str()).collect();

    // Build adjacency list
    let mut adj: FxHashMap<&str, Vec<&str>> = FxHashMap::default();
    for dep in dependencies {
        if module_ids.contains(dep.from_module_id.as_str())
            && module_ids.contains(dep.to_module_id.as_str())
        {
            adj.entry(dep.from_module_id.as_str())
                .or_default()
                .push(dep.to_module_id.as_str());
        }
    }

    // DFS-based cycle detection
    let mut visited: FxHashSet<&str> = FxHashSet::default();
    let mut in_stack: FxHashSet<&str> = FxHashSet::default();
    let mut cycle_edges: Vec<(String, String)> = Vec::new();

    for module in modules {
        if !visited.contains(module.id.as_str()) {
            dfs_detect(
                module.id.as_str(),
                &adj,
                &mut visited,
                &mut in_stack,
                &mut cycle_edges,
            );
        }
    }

    cycle_edges
}

fn dfs_detect<'a>(
    node: &'a str,
    adj: &FxHashMap<&'a str, Vec<&'a str>>,
    visited: &mut FxHashSet<&'a str>,
    in_stack: &mut FxHashSet<&'a str>,
    cycle_edges: &mut Vec<(String, String)>,
) {
    visited.insert(node);
    in_stack.insert(node);

    if let Some(neighbors) = adj.get(node) {
        for &next in neighbors {
            if !visited.contains(next) {
                dfs_detect(next, adj, visited, in_stack, cycle_edges);
            } else if in_stack.contains(next) {
                cycle_edges.push((node.to_string(), next.to_string()));
            }
        }
    }

    in_stack.remove(node);
}

/// Run the full audit: D_acc aggregation, BFS coupling detection, severity, and health score.
pub fn audit(modules: &[Module], dependencies: &[Dependency]) -> AuditResult {
    if modules.is_empty() {
        return AuditResult {
            violations: Vec::new(),
            score: 100.0,
            total_modules: 0,
        };
    }

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

    // Detect circular dependencies (module-level cycles)
    let circular = detect_cycles(modules, dependencies);
    for (from_id, to_id) in &circular {
        let from_path = id_to_path.get(from_id.as_str()).unwrap_or(&"");
        let to_path = id_to_path.get(to_id.as_str()).unwrap_or(&"");
        let from_dir = id_to_dir.get(from_id.as_str()).cloned().unwrap_or_default();
        let to_dir = id_to_dir.get(to_id.as_str()).cloned().unwrap_or_default();
        violations.push(CouplingViolation {
            dir_a: from_dir,
            dir_b: to_dir,
            from_module: from_path.to_string(),
            to_module: to_path.to_string(),
            depth: 0,
            severity: 1.0,
            is_circular: true,
        });
    }

    // BFS: for each parent directory, check sibling pairs
    for (parent, children) in &dir_tree {
        let depth = if parent.is_empty() {
            0
        } else {
            dir_depth(parent) + 1
        };

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
                                                    from_module: id_to_path.get(dep.from_module_id.as_str()).unwrap_or(&"").to_string(),
                                                    to_module: id_to_path.get(dep.to_module_id.as_str()).unwrap_or(&"").to_string(),
                                                    depth,
                                                    severity: 1.0 / (depth as f64 + 1.0),
                                                    is_circular: false,
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
                                                    from_module: id_to_path.get(dep.from_module_id.as_str()).unwrap_or(&"").to_string(),
                                                    to_module: id_to_path.get(dep.to_module_id.as_str()).unwrap_or(&"").to_string(),
                                                    depth,
                                                    severity: 1.0 / (depth as f64 + 1.0),
                                                    is_circular: false,
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

    // Sort by severity descending
    violations.sort_by(|a, b| b.severity.partial_cmp(&a.severity).unwrap());

    // Dedup violations (same from_module + to_module pair at same depth)
    violations.dedup_by(|a, b| {
        a.from_module == b.from_module && a.to_module == b.to_module && a.depth == b.depth
    });

    // Health score
    let sum_severity: f64 = violations.iter().map(|v| v.severity).sum();
    let total_modules = modules.len();
    let score = (100.0 * (1.0 - sum_severity / total_modules as f64)).max(0.0);

    AuditResult {
        violations,
        score,
        total_modules,
    }
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

    // ── BFS coupling detection ──

    #[test]
    fn detects_sibling_coupling() {
        // scanner depends on storage (siblings under src/slices)
        let modules = vec![
            make_module("a", "src/slices/scanner/mod.rs"),
            make_module("b", "src/slices/storage/mod.rs"),
        ];
        let deps = vec![make_dep("a", "b", 10)];

        let result = audit(&modules, &deps);
        assert!(!result.violations.is_empty(), "Should detect coupling between scanner and storage");
        assert_eq!(result.violations[0].dir_a, "src/slices/scanner");
        assert_eq!(result.violations[0].dir_b, "src/slices/storage");
    }

    #[test]
    fn no_violations_when_independent() {
        let modules = vec![
            make_module("a", "src/slices/scanner/mod.rs"),
            make_module("b", "src/slices/storage/mod.rs"),
        ];
        let deps: Vec<Dependency> = vec![];

        let result = audit(&modules, &deps);
        assert!(result.violations.is_empty());
    }

    #[test]
    fn no_violations_for_internal_deps() {
        let modules = vec![
            make_module("a", "src/scanner/parser.rs"),
            make_module("b", "src/scanner/resolver.rs"),
        ];
        let deps = vec![make_dep("a", "b", 1)];

        let result = audit(&modules, &deps);
        assert!(result.violations.is_empty());
    }

    // ── Severity ──

    #[test]
    fn severity_at_depth_zero() {
        // Two top-level sibling dirs
        let modules = vec![
            make_module("a", "scanner/mod.rs"),
            make_module("b", "storage/mod.rs"),
        ];
        let deps = vec![make_dep("a", "b", 1)];

        let result = audit(&modules, &deps);
        assert!(!result.violations.is_empty());
        assert!((result.violations[0].severity - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn severity_decreases_with_depth() {
        // Siblings at depth 2 (under src/slices/)
        let modules = vec![
            make_module("a", "src/slices/scanner/mod.rs"),
            make_module("b", "src/slices/storage/mod.rs"),
        ];
        let deps = vec![make_dep("a", "b", 1)];

        let result = audit(&modules, &deps);
        assert!(!result.violations.is_empty());
        // parent "src/slices" has depth 2, children are at depth 3, severity = 1/(3+1) = 0.25
        let expected = 0.25;
        assert!(
            (result.violations[0].severity - expected).abs() < 0.01,
            "Expected severity ~{}, got {}",
            expected,
            result.violations[0].severity
        );
    }

    // ── Health score ──

    #[test]
    fn perfect_score_no_violations() {
        let modules = vec![
            make_module("a", "src/scanner/mod.rs"),
            make_module("b", "src/storage/mod.rs"),
        ];
        let deps: Vec<Dependency> = vec![];

        let result = audit(&modules, &deps);
        assert!((result.score - 100.0).abs() < f64::EPSILON);
    }

    #[test]
    fn score_decreases_with_violations() {
        let modules = vec![
            make_module("a", "scanner/mod.rs"),
            make_module("b", "storage/mod.rs"),
        ];
        let deps = vec![make_dep("a", "b", 1)];

        let result = audit(&modules, &deps);
        assert!(result.score < 100.0);
        assert!(result.score >= 0.0);
        // severity=1.0, total_modules=2, score=100*(1-1.0/2)=50
        assert!((result.score - 50.0).abs() < 0.01, "Expected ~50, got {}", result.score);
    }

    #[test]
    fn score_clamps_to_zero() {
        // Many high-severity violations
        let modules = vec![
            make_module("a", "x/mod.rs"),
            make_module("b", "y/mod.rs"),
        ];
        // Create multiple deps to push score below 0
        let deps = vec![
            make_dep("a", "b", 1),
            make_dep("a", "b", 2),
            make_dep("a", "b", 3),
            make_dep("b", "a", 1),
            make_dep("b", "a", 2),
            make_dep("b", "a", 3),
        ];

        let result = audit(&modules, &deps);
        assert!(result.score >= 0.0);
    }

    #[test]
    fn empty_project_scores_100() {
        let result = audit(&[], &[]);
        assert!((result.score - 100.0).abs() < f64::EPSILON);
    }

    #[test]
    fn violations_sorted_by_severity_descending() {
        let modules = vec![
            make_module("a", "scanner/mod.rs"),
            make_module("b", "storage/mod.rs"),
            make_module("c", "src/slices/analyzer/mod.rs"),
            make_module("d", "src/slices/reporter/mod.rs"),
        ];
        let deps = vec![
            make_dep("a", "b", 1),  // depth 0, severity 1.0
            make_dep("c", "d", 1),  // depth 2, severity 0.33
        ];

        let result = audit(&modules, &deps);
        if result.violations.len() >= 2 {
            assert!(result.violations[0].severity >= result.violations[1].severity);
        }
    }

    // ── Circular dependencies ──

    #[test]
    fn detects_circular_dependency() {
        let modules = vec![
            make_module("a", "src/alpha/mod.rs"),
            make_module("b", "src/beta/mod.rs"),
        ];
        // A -> B and B -> A = cycle
        let deps = vec![make_dep("a", "b", 1), make_dep("b", "a", 5)];

        let result = audit(&modules, &deps);
        let circular: Vec<&CouplingViolation> =
            result.violations.iter().filter(|v| v.is_circular).collect();
        assert!(
            !circular.is_empty(),
            "Should detect circular dependency between a and b"
        );
        assert!((circular[0].severity - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn no_circular_when_one_direction() {
        let modules = vec![
            make_module("a", "src/alpha/mod.rs"),
            make_module("b", "src/beta/mod.rs"),
        ];
        let deps = vec![make_dep("a", "b", 1)];

        let result = audit(&modules, &deps);
        let circular: Vec<&CouplingViolation> =
            result.violations.iter().filter(|v| v.is_circular).collect();
        assert!(circular.is_empty());
    }

    #[test]
    fn detects_transitive_cycle() {
        let modules = vec![
            make_module("a", "src/x/mod.rs"),
            make_module("b", "src/y/mod.rs"),
            make_module("c", "src/z/mod.rs"),
        ];
        // A -> B -> C -> A = transitive cycle
        let deps = vec![
            make_dep("a", "b", 1),
            make_dep("b", "c", 1),
            make_dep("c", "a", 1),
        ];

        let result = audit(&modules, &deps);
        let circular: Vec<&CouplingViolation> =
            result.violations.iter().filter(|v| v.is_circular).collect();
        assert!(
            !circular.is_empty(),
            "Should detect transitive circular dependency"
        );
    }
}
