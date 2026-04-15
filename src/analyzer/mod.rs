//! Architectural analysis engine.
//!
//! Computes coupling violations and circular dependencies using
//! bottom-up D_acc aggregation and top-down BFS sibling analysis.

use fxhash::{FxHashMap, FxHashSet};
use std::collections::BTreeMap;

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
    /// Whether this is a circular dependency (vs. a coupling violation).
    pub is_circular: bool,
    /// For circular deps: the full cycle path as directory paths.
    pub cycle_path: Vec<String>,
    /// For circular deps: `(from_file, to_file, line_number)` for each hop in the cycle.
    pub cycle_hop_files: Vec<(String, String, i32)>,
    /// For circular deps: number of nodes in the cycle (2 = mutual, 3 = triangle, etc.).
    pub cycle_order: usize,
    /// For circular deps: the weakest hop to break (fewest imports). Format: "dir_a -> dir_b (N imports)".
    pub weakest_link: Option<String>,
    /// For circular deps: number of imports to remove at the weakest link to break the cycle.
    pub break_cost: usize,
}

/// A module's dependency metrics.
#[derive(Debug, Clone)]
pub struct ModuleMetrics {
    /// Relative path of the module.
    pub path: String,
    /// Number of other modules that import this module (incoming).
    pub fan_in: usize,
    /// Number of modules this module imports (outgoing).
    pub fan_out: usize,
}

/// The result of running an architectural audit on a project snapshot.
#[derive(Debug)]
pub struct AuditResult {
    /// All detected violations, sorted by severity descending.
    pub violations: Vec<CouplingViolation>,
    /// Overall health score (0-100). Higher is better.
    pub score: f64,
    /// Total number of source modules analyzed.
    pub total_modules: usize,
    /// Per-module fan-in/fan-out metrics, sorted by fan_in descending.
    pub hotspots: Vec<ModuleMetrics>,
    /// Violations of custom dependency rules from settings.json.
    pub rule_violations: Vec<RuleViolation>,
    /// Violations of architectural layer ordering.
    pub layer_violations: Vec<LayerViolation>,
    /// Per-directory cohesion metrics.
    pub cohesion: Vec<CohesionMetrics>,
    /// Total excess: sum of all imports that need removal to fix all violations.
    pub total_xs: usize,
}

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

/// A violation of a custom dependency rule.
#[derive(Debug, Clone)]
pub struct RuleViolation {
    /// Source file path.
    pub from_module: String,
    /// Target file path.
    pub to_module: String,
    /// Line number of the import.
    pub line_number: i32,
    /// Custom message from the rule definition.
    pub message: String,
}

/// A violation of architectural layer ordering.
#[derive(Debug, Clone)]
pub struct LayerViolation {
    /// Source file path.
    pub from_module: String,
    /// Target file path.
    pub to_module: String,
    /// Line number of the import.
    pub line_number: i32,
    /// Layer of the source module.
    pub from_layer: String,
    /// Layer of the target module (higher layer being imported).
    pub to_layer: String,
}

impl AuditResult {
    /// Keep only violations involving at least one changed file and recalculate the score.
    pub fn filter_by_changed_files(&mut self, changed_files: &[String]) {
        self.violations.retain(|v| {
            // Coupling: check if from_module or to_module is a changed file
            if !v.is_circular {
                return changed_files
                    .iter()
                    .any(|f| v.from_module.ends_with(f) || f.ends_with(&v.from_module))
                    || changed_files
                        .iter()
                        .any(|f| v.to_module.ends_with(f) || f.ends_with(&v.to_module));
            }
            // Circular: check if any hop file in the cycle is a changed file
            for (from_file, to_file, _) in &v.cycle_hop_files {
                if changed_files
                    .iter()
                    .any(|f| from_file.ends_with(f) || f.ends_with(from_file))
                {
                    return true;
                }
                if changed_files
                    .iter()
                    .any(|f| to_file.ends_with(f) || f.ends_with(to_file))
                {
                    return true;
                }
            }
            false
        });
        self.recalculate_score();
    }

    /// Remove coupling violations that follow the defined layer direction (downward).
    /// Keeps circular violations and violations where modules have no layer assigned.
    pub fn filter_by_layers(&mut self, layers: &[crate::settings::Layer]) {
        if layers.is_empty() {
            return;
        }

        // Build layer matchers and index
        let layer_matchers: Vec<(usize, globset::GlobMatcher)> = layers
            .iter()
            .enumerate()
            .filter_map(|(i, l)| {
                globset::Glob::new(&l.pattern)
                    .ok()
                    .map(|g| (i, g.compile_matcher()))
            })
            .collect();

        let get_layer = |path: &str| -> Option<usize> {
            for (idx, matcher) in &layer_matchers {
                if matcher.is_match(path) {
                    return Some(*idx);
                }
            }
            None
        };

        self.violations.retain(|v| {
            // Always keep circular violations
            if v.is_circular {
                return true;
            }

            let from_layer = get_layer(&v.from_module);
            let to_layer = get_layer(&v.to_module);

            match (from_layer, to_layer) {
                // Both have layers: suppress downward deps (from_idx < to_idx)
                // Keep: same layer (from_idx == to_idx) or upward (from_idx > to_idx)
                (Some(from_idx), Some(to_idx)) => from_idx >= to_idx,
                // One or both unassigned: keep the violation
                _ => true,
            }
        });
        self.recalculate_score();
    }

    /// Remove violations below the given severity threshold and recalculate the score.
    pub fn filter_by_severity(&mut self, minimum_severity: f64) {
        // Circular violations are always kept regardless of severity
        self.violations
            .retain(|v| v.is_circular || v.severity >= minimum_severity);
        self.recalculate_score();
    }

    pub fn recalculate_score(&mut self) {
        let sum_severity: f64 = self.violations.iter().map(|v| v.severity).sum();
        self.score = if self.total_modules > 0 {
            (100.0 * (1.0 - sum_severity / self.total_modules as f64)).max(0.0)
        } else {
            100.0
        };
    }
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

/// A detected cycle at the directory level with the full path of directories involved.
struct DetectedCycle {
    /// Full cycle path as directory paths (e.g., ["src/main", "src/analytics", "src/deeplink", "src/main"])
    dir_path: Vec<String>,
    /// For each hop in the cycle, the file that causes the dependency (from_file -> to_file)
    /// Length is dir_path.len() - 1 (one edge per hop)
    hop_files: Vec<(String, String, i32)>,
    /// For each hop, how many imports cross that edge. Used to find the weakest link.
    hop_import_counts: Vec<usize>,
}

/// Detect circular dependencies among sibling directories using their D_acc sets.
/// A cycle exists when sibling A's D_acc points into B, B's into C, ..., and back to A.
fn detect_sibling_cycles(
    siblings: &[String],
    dacc: &FxHashMap<String, FxHashSet<String>>,
    id_to_dir: &FxHashMap<&str, String>,
    id_to_path: &FxHashMap<&str, &str>,
    dependencies: &[Dependency],
) -> Vec<DetectedCycle> {
    if siblings.len() < 2 {
        return Vec::new();
    }

    // Build adjacency: sibling A -> sibling B if D_acc(A) contains a module under B
    let mut adj: FxHashMap<usize, Vec<usize>> = FxHashMap::default();
    // Track which file causes each edge for hop_files (first match only, for display)
    let mut edge_files: FxHashMap<(usize, usize), (String, String, i32)> = FxHashMap::default();
    // Track total import count per edge for XS metric (weakest_link / break_cost)
    let mut edge_import_count: FxHashMap<(usize, usize), usize> = FxHashMap::default();

    for (i, dir_a) in siblings.iter().enumerate() {
        if let Some(deps_a) = dacc.get(dir_a) {
            for target_id in deps_a {
                if let Some(target_dir) = id_to_dir.get(target_id.as_str()) {
                    for (j, dir_b) in siblings.iter().enumerate() {
                        if i == j {
                            continue;
                        }
                        let b_prefix = format!("{}/", dir_b);
                        if target_dir == dir_b || target_dir.starts_with(&b_prefix) {
                            adj.entry(i).or_default().push(j);
                            // Count all imports for this edge
                            let a_prefix = format!("{}/", dir_a);
                            for dep in dependencies {
                                if &dep.to_module_id == target_id {
                                    let from_dir = id_to_dir.get(dep.from_module_id.as_str());
                                    if let Some(fd) = from_dir {
                                        if fd == dir_a || fd.starts_with(&a_prefix) {
                                            *edge_import_count.entry((i, j)).or_insert(0) += 1;
                                            edge_files.entry((i, j)).or_insert_with(|| {
                                                let from_file = id_to_path
                                                    .get(dep.from_module_id.as_str())
                                                    .unwrap_or(&"")
                                                    .to_string();
                                                let to_file = id_to_path
                                                    .get(dep.to_module_id.as_str())
                                                    .unwrap_or(&"")
                                                    .to_string();
                                                (from_file, to_file, dep.line_number)
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

    // Deduplicate adjacency
    for targets in adj.values_mut() {
        targets.sort();
        targets.dedup();
    }

    // Find all elementary cycles using DFS from each node
    let mut cycles: Vec<DetectedCycle> = Vec::new();
    let mut seen_keys: FxHashSet<String> = FxHashSet::default();

    for start in 0..siblings.len() {
        let mut path = vec![start];
        let mut visited_in_path: FxHashSet<usize> = FxHashSet::default();
        visited_in_path.insert(start);
        find_all_cycles_from(
            start,
            start,
            &adj,
            &mut path,
            &mut visited_in_path,
            &mut cycles,
            &mut seen_keys,
            siblings,
            &edge_files,
            &edge_import_count,
        );
    }

    cycles
}

#[allow(clippy::too_many_arguments)]
fn find_all_cycles_from(
    start: usize,
    current: usize,
    adj: &FxHashMap<usize, Vec<usize>>,
    path: &mut Vec<usize>,
    visited: &mut FxHashSet<usize>,
    cycles: &mut Vec<DetectedCycle>,
    seen_keys: &mut FxHashSet<String>,
    siblings: &[String],
    edge_files: &FxHashMap<(usize, usize), (String, String, i32)>,
    edge_import_count: &FxHashMap<(usize, usize), usize>,
) {
    if let Some(neighbors) = adj.get(&current) {
        for &next in neighbors {
            if next == start && path.len() >= 2 {
                // Found a cycle back to start
                let mut sorted = path.clone();
                sorted.sort();
                let key = sorted
                    .iter()
                    .map(|i| i.to_string())
                    .collect::<Vec<_>>()
                    .join(",");
                if !seen_keys.contains(&key) {
                    seen_keys.insert(key);

                    let mut dir_path: Vec<String> =
                        path.iter().map(|&i| siblings[i].clone()).collect();
                    dir_path.push(siblings[start].clone());

                    let mut hop_files = Vec::new();
                    let mut hop_import_counts = Vec::new();
                    for w in 0..path.len() {
                        let from_idx = path[w];
                        let to_idx = if w + 1 < path.len() {
                            path[w + 1]
                        } else {
                            start
                        };
                        let files = edge_files
                            .get(&(from_idx, to_idx))
                            .cloned()
                            .unwrap_or_default();
                        hop_files.push(files);
                        let count = edge_import_count
                            .get(&(from_idx, to_idx))
                            .copied()
                            .unwrap_or(1);
                        hop_import_counts.push(count);
                    }

                    cycles.push(DetectedCycle {
                        dir_path,
                        hop_files,
                        hop_import_counts,
                    });
                }
            } else if !visited.contains(&next) && next > start {
                // Only explore nodes with index > start to avoid finding the same cycle
                // from different starting points
                visited.insert(next);
                path.push(next);
                find_all_cycles_from(
                    start, next, adj, path, visited, cycles, seen_keys, siblings,
                    edge_files, edge_import_count,
                );
                path.pop();
                visited.remove(&next);
            }
        }
    }
}

/// Run the full audit: D_acc aggregation, BFS coupling detection, severity, and health score.
pub fn audit(modules: &[Module], dependencies: &[Dependency]) -> AuditResult {
    if modules.is_empty() {
        return AuditResult {
            violations: Vec::new(),
            score: 100.0,
            total_modules: 0,
            hotspots: Vec::new(),
            rule_violations: Vec::new(),
            layer_violations: Vec::new(),
            cohesion: Vec::new(), total_xs: 0,
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
                is_circular: true,
                cycle_path: cycle.dir_path.clone(),
                cycle_hop_files: cycle.hop_files.clone(),
                cycle_order: cycle.dir_path.len() - 1, // -1 because last entry closes the cycle
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
                                                    is_circular: false,
                                                    cycle_path: Vec::new(),
                                                    cycle_hop_files: Vec::new(),
                                                    cycle_order: 0, weakest_link: None, break_cost: 0,
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
                                                    is_circular: false,
                                                    cycle_path: Vec::new(),
                                                    cycle_hop_files: Vec::new(),
                                                    cycle_order: 0, weakest_link: None, break_cost: 0,
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

    // Health score
    let sum_severity: f64 = violations.iter().map(|v| v.severity).sum();
    let total_modules = modules.len();
    let score = (100.0 * (1.0 - sum_severity / total_modules as f64)).max(0.0);

    // Compute hotspots (fan-in / fan-out per module)
    let mut fan_in: FxHashMap<&str, usize> = FxHashMap::default();
    let mut fan_out: FxHashMap<&str, usize> = FxHashMap::default();
    for dep in dependencies {
        *fan_in.entry(dep.to_module_id.as_str()).or_insert(0) += 1;
        *fan_out.entry(dep.from_module_id.as_str()).or_insert(0) += 1;
    }
    let mut hotspots: Vec<ModuleMetrics> = modules
        .iter()
        .map(|m| ModuleMetrics {
            path: m.path.clone(),
            fan_in: *fan_in.get(m.id.as_str()).unwrap_or(&0),
            fan_out: *fan_out.get(m.id.as_str()).unwrap_or(&0),
        })
        .collect();
    hotspots.sort_by(|a, b| b.fan_in.cmp(&a.fan_in));

    // Compute per-directory cohesion
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

    // Calculate total XS: sum of weights for coupling + break_cost for circular
    let total_xs: usize = violations
        .iter()
        .map(|v| if v.is_circular { v.break_cost } else { v.weight })
        .sum();

    AuditResult {
        violations,
        score,
        total_modules,
        hotspots,
        rule_violations: Vec::new(),
        layer_violations: Vec::new(),
        cohesion,
        total_xs,
    }
}

/// Check dependencies against custom rules from settings.json.
pub fn check_dependency_rules(
    modules: &[Module],
    dependencies: &[Dependency],
    rules: &[crate::settings::DependencyRule],
) -> Vec<RuleViolation> {
    if rules.is_empty() {
        return Vec::new();
    }

    let id_to_path: FxHashMap<&str, &str> = modules
        .iter()
        .map(|m| (m.id.as_str(), m.path.as_str()))
        .collect();

    let mut violations = Vec::new();

    for rule in rules {
        if rule.allow {
            continue; // Only check forbidden rules
        }

        let from_glob = match globset::Glob::new(&rule.from) {
            Ok(g) => g.compile_matcher(),
            Err(_) => continue,
        };
        let to_glob = match globset::Glob::new(&rule.to) {
            Ok(g) => g.compile_matcher(),
            Err(_) => continue,
        };

        for dep in dependencies {
            let from_path = match id_to_path.get(dep.from_module_id.as_str()) {
                Some(p) => *p,
                None => continue,
            };
            let to_path = match id_to_path.get(dep.to_module_id.as_str()) {
                Some(p) => *p,
                None => continue,
            };

            if from_glob.is_match(from_path) && to_glob.is_match(to_path) {
                violations.push(RuleViolation {
                    from_module: from_path.to_string(),
                    to_module: to_path.to_string(),
                    line_number: dep.line_number,
                    message: if rule.message.is_empty() {
                        format!("Forbidden dependency: {} -> {}", rule.from, rule.to)
                    } else {
                        rule.message.clone()
                    },
                });
            }
        }
    }

    violations
}

/// Check dependencies against architectural layer ordering.
/// Dependencies may only flow downward (higher index = lower layer).
pub fn check_layer_rules(
    modules: &[Module],
    dependencies: &[Dependency],
    layers: &[crate::settings::Layer],
) -> Vec<LayerViolation> {
    if layers.is_empty() {
        return Vec::new();
    }

    // Build glob matchers for each layer
    let layer_matchers: Vec<(usize, &str, globset::GlobMatcher)> = layers
        .iter()
        .enumerate()
        .filter_map(|(i, l)| {
            globset::Glob::new(&l.pattern)
                .ok()
                .map(|g| (i, l.name.as_str(), g.compile_matcher()))
        })
        .collect();

    let id_to_path: FxHashMap<&str, &str> = modules
        .iter()
        .map(|m| (m.id.as_str(), m.path.as_str()))
        .collect();

    // Assign each module to a layer (first matching pattern wins)
    let mut module_layer: FxHashMap<&str, (usize, &str)> = FxHashMap::default();
    for module in modules {
        for (idx, name, matcher) in &layer_matchers {
            if matcher.is_match(&module.path) {
                module_layer.insert(module.id.as_str(), (*idx, name));
                break;
            }
        }
    }

    let mut violations = Vec::new();

    for dep in dependencies {
        let from_layer = module_layer.get(dep.from_module_id.as_str());
        let to_layer = module_layer.get(dep.to_module_id.as_str());

        if let (Some((from_idx, _from_name)), Some((to_idx, to_name))) = (from_layer, to_layer) {
            // Violation: importing from a higher layer (lower index = higher layer)
            if to_idx < from_idx {
                let from_path = id_to_path.get(dep.from_module_id.as_str()).unwrap_or(&"");
                let to_path = id_to_path.get(dep.to_module_id.as_str()).unwrap_or(&"");
                let from_name = module_layer
                    .get(dep.from_module_id.as_str())
                    .map(|(_, n)| *n)
                    .unwrap_or("");
                violations.push(LayerViolation {
                    from_module: from_path.to_string(),
                    to_module: to_path.to_string(),
                    line_number: dep.line_number,
                    from_layer: from_name.to_string(),
                    to_layer: to_name.to_string(),
                });
            }
        }
    }

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
        assert!(
            !result.violations.is_empty(),
            "Should detect coupling between scanner and storage"
        );
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
        assert!(
            (result.score - 50.0).abs() < 0.01,
            "Expected ~50, got {}",
            result.score
        );
    }

    #[test]
    fn score_clamps_to_zero() {
        // Many high-severity violations
        let modules = vec![make_module("a", "x/mod.rs"), make_module("b", "y/mod.rs")];
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
            make_dep("a", "b", 1), // depth 0, severity 1.0
            make_dep("c", "d", 1), // depth 2, severity 0.33
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
        // Severity depends on depth: siblings under "src" are at depth 2, severity = 1/(2+1)
        assert!(circular[0].severity > 0.0);
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
