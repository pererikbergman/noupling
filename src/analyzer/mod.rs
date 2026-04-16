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
    /// For circular deps: number of imports crossing each hop in the cycle.
    pub cycle_hop_counts: Vec<usize>,
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
    /// Martin's Instability: fan_out / (fan_in + fan_out). Range 0.0 (stable) to 1.0 (unstable).
    pub instability: f64,
    /// Number of modules transitively affected if this module changes.
    pub blast_radius: usize,
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
    /// Per-module independence scores (internal vs external dependency ratio).
    pub independence: Vec<ModuleIndependence>,
    /// Maximum dependency chain depth and the critical path.
    pub max_depth: usize,
    /// The longest dependency chain (file paths from root to deepest leaf).
    pub critical_path: Vec<String>,
    /// Violation age summary: new, recent, chronic counts.
    pub violation_age: ViolationAgeSummary,
    /// Sibling coupling pairs tracked as metrics (not violations) in actionable mode.
    pub coupling_metrics_count: usize,
    /// Number of imports suppressed by `noupling:ignore` comments.
    pub suppressed_count: usize,
}

/// Summary of violation ages across snapshots.
#[derive(Debug, Clone, Default)]
pub struct ViolationAgeSummary {
    /// Violations that first appeared in the latest snapshot.
    pub new_count: usize,
    /// Violations that have existed for 2-4 snapshots.
    pub recent_count: usize,
    /// Violations that have existed for 5+ snapshots.
    pub chronic_count: usize,
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

    /// In "actionable" coupling mode, sibling coupling violations are not
    /// counted as violations — only circular dependencies remain in the
    /// `violations` list. Layer/rule/cross-module violations are tracked
    /// separately and unaffected.
    ///
    /// Sibling coupling is still measured (cohesion, hotspots, weights) but
    /// no longer treated as a violation that drags down the score.
    pub fn apply_coupling_mode(&mut self, mode: &str) {
        if mode == "actionable" {
            self.coupling_metrics_count = self.violations.iter().filter(|v| !v.is_circular).count();
            self.violations.retain(|v| v.is_circular);
            self.total_xs = self
                .violations
                .iter()
                .map(|v| {
                    if v.is_circular {
                        v.break_cost
                    } else {
                        v.weight
                    }
                })
                .sum();
            self.recalculate_score();
        }
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

    // Find SCCs (Strongly Connected Components) via Tarjan's algorithm.
    // Each SCC with size >= 2 represents one circular dependency.
    // This is O(V+E) and avoids the combinatorial blowup of enumerating
    // all elementary cycles.
    let sccs = find_sccs(&adj, siblings.len());

    let mut cycles: Vec<DetectedCycle> = Vec::new();
    for scc in sccs {
        if scc.len() < 2 {
            continue;
        }
        // Build a representative cycle path through the SCC.
        // Walk the nodes in order, finding edges that exist in adj.
        let scc_set: FxHashSet<usize> = scc.iter().copied().collect();
        let mut ordered: Vec<usize> = Vec::new();
        let mut visited_in_path: FxHashSet<usize> = FxHashSet::default();

        // Try to construct a Hamiltonian-like cycle: start at first node,
        // greedily pick neighbors within the SCC.
        let start = scc[0];
        ordered.push(start);
        visited_in_path.insert(start);
        let mut current = start;
        loop {
            let next = adj.get(&current).and_then(|neighbors| {
                neighbors
                    .iter()
                    .find(|&&n| scc_set.contains(&n) && !visited_in_path.contains(&n))
                    .copied()
            });
            match next {
                Some(n) => {
                    ordered.push(n);
                    visited_in_path.insert(n);
                    current = n;
                }
                None => break,
            }
        }
        // Add any remaining SCC nodes that weren't reached (rare)
        for &n in &scc {
            if !visited_in_path.contains(&n) {
                ordered.push(n);
            }
        }

        // Build dir_path with the cycle closed by repeating the start
        let mut dir_path: Vec<String> = ordered.iter().map(|&i| siblings[i].clone()).collect();
        dir_path.push(siblings[start].clone());

        // Build hop_files and hop_import_counts using adj
        let mut hop_files = Vec::new();
        let mut hop_import_counts = Vec::new();
        for w in 0..ordered.len() {
            let from_idx = ordered[w];
            let to_idx = if w + 1 < ordered.len() {
                ordered[w + 1]
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

    cycles
}

/// Tarjan's strongly connected components algorithm (iterative).
fn find_sccs(adj: &FxHashMap<usize, Vec<usize>>, n: usize) -> Vec<Vec<usize>> {
    let mut index_counter: i32 = 0;
    let mut indices: Vec<i32> = vec![-1; n];
    let mut lowlinks: Vec<i32> = vec![-1; n];
    let mut on_stack: Vec<bool> = vec![false; n];
    let mut stack: Vec<usize> = Vec::new();
    let mut sccs: Vec<Vec<usize>> = Vec::new();

    let empty: Vec<usize> = Vec::new();

    for v in 0..n {
        if indices[v] != -1 {
            continue;
        }
        // Iterative strongconnect
        let mut work_stack: Vec<(usize, usize)> = vec![(v, 0)];
        indices[v] = index_counter;
        lowlinks[v] = index_counter;
        index_counter += 1;
        stack.push(v);
        on_stack[v] = true;

        while let Some(&(node, neighbor_idx)) = work_stack.last() {
            let neighbors = adj.get(&node).unwrap_or(&empty);
            if neighbor_idx < neighbors.len() {
                let w = neighbors[neighbor_idx];
                // Advance pointer
                if let Some(top) = work_stack.last_mut() {
                    top.1 += 1;
                }

                if indices[w] == -1 {
                    indices[w] = index_counter;
                    lowlinks[w] = index_counter;
                    index_counter += 1;
                    stack.push(w);
                    on_stack[w] = true;
                    work_stack.push((w, 0));
                } else if on_stack[w] {
                    lowlinks[node] = lowlinks[node].min(indices[w]);
                }
            } else {
                // All neighbors processed
                work_stack.pop();
                if lowlinks[node] == indices[node] {
                    // Root of SCC — pop everything until node
                    let mut scc = Vec::new();
                    loop {
                        let w = stack.pop().expect("stack should not be empty");
                        on_stack[w] = false;
                        scc.push(w);
                        if w == node {
                            break;
                        }
                    }
                    sccs.push(scc);
                }
                // Propagate lowlink to parent
                if let Some(&(parent, _)) = work_stack.last() {
                    lowlinks[parent] = lowlinks[parent].min(lowlinks[node]);
                }
            }
        }
    }

    sccs
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
            cohesion: Vec::new(),
            total_xs: 0,
            independence: Vec::new(),
            max_depth: 0,
            critical_path: Vec::new(),
            violation_age: ViolationAgeSummary::default(),
            coupling_metrics_count: 0,
            suppressed_count: 0,
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

    // Health score
    let sum_severity: f64 = violations.iter().map(|v| v.severity).sum();
    let total_modules = modules.len();
    let score = (100.0 * (1.0 - sum_severity / total_modules as f64)).max(0.0);

    // Compute hotspots (fan-in / fan-out / instability / blast radius per module)
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

    // Compute per-module independence (top-level directories)
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

    // Calculate total XS: sum of weights for coupling + break_cost for circular
    let total_xs: usize = violations
        .iter()
        .map(|v| {
            if v.is_circular {
                v.break_cost
            } else {
                v.weight
            }
        })
        .sum();

    // Compute dependency depth via longest path (DP on forward graph)
    let id_to_idx: FxHashMap<&str, usize> = modules
        .iter()
        .enumerate()
        .map(|(i, m)| (m.id.as_str(), i))
        .collect();
    let mut adj: Vec<Vec<usize>> = vec![Vec::new(); modules.len()];
    for dep in dependencies {
        if let (Some(&from), Some(&to)) = (
            id_to_idx.get(dep.from_module_id.as_str()),
            id_to_idx.get(dep.to_module_id.as_str()),
        ) {
            adj[from].push(to);
        }
    }

    // Deduplicate adjacency lists
    for list in &mut adj {
        list.sort();
        list.dedup();
    }

    // Iterative longest path using Kahn's algorithm (topological sort)
    // For nodes in cycles, depth stays 0.
    let n = modules.len();
    let mut in_degree = vec![0usize; n];
    for edges in &adj {
        for &to in edges {
            in_degree[to] += 1;
        }
    }

    // Reverse topological order via BFS (Kahn's)
    let mut queue: std::collections::VecDeque<usize> = std::collections::VecDeque::new();
    let mut depth_val: Vec<usize> = vec![0; n];
    let mut parent_of: Vec<Option<usize>> = vec![None; n];

    // Start from leaves (in_degree == 0 in reverse graph = out_degree == 0)
    // Actually, we want longest path FROM each node, so we process in reverse topo order.
    // Use reverse graph + forward BFS.
    let mut rev_adj: Vec<Vec<usize>> = vec![Vec::new(); n];
    for (from, edges) in adj.iter().enumerate() {
        for &to in edges {
            rev_adj[to].push(from);
        }
    }

    let mut out_degree: Vec<usize> = adj.iter().map(|e| e.len()).collect();
    for (i, &deg) in out_degree.iter().enumerate() {
        if deg == 0 {
            queue.push_back(i);
        }
    }

    while let Some(node) = queue.pop_front() {
        for &pred in &rev_adj[node] {
            let new_depth = depth_val[node] + 1;
            if new_depth > depth_val[pred] {
                depth_val[pred] = new_depth;
                parent_of[pred] = Some(node);
            }
            out_degree[pred] -= 1;
            if out_degree[pred] == 0 {
                queue.push_back(pred);
            }
        }
    }

    // Find the node with the longest path
    let (max_depth, start_idx) = depth_val
        .iter()
        .enumerate()
        .max_by_key(|(_, &d)| d)
        .map(|(i, &d)| (d, i))
        .unwrap_or((0, 0));

    // Reconstruct the critical path
    let mut critical_path = Vec::new();
    if max_depth > 0 {
        let mut current = start_idx;
        critical_path.push(modules[current].path.clone());
        while let Some(next) = parent_of[current] {
            critical_path.push(modules[next].path.clone());
            current = next;
        }
    }

    AuditResult {
        violations,
        score,
        total_modules,
        hotspots,
        rule_violations: Vec::new(),
        layer_violations: Vec::new(),
        cohesion,
        total_xs,
        independence,
        max_depth,
        critical_path,
        violation_age: ViolationAgeSummary::default(),
        coupling_metrics_count: 0,
        suppressed_count: 0,
    }
}

/// Compute violation ages by comparing current violations against historical snapshots.
/// Returns an updated ViolationAgeSummary.
pub fn compute_violation_age(
    current_violations: &[CouplingViolation],
    historical_violation_sets: &[Vec<(String, String)>], // Vec of (from_module, to_module) per snapshot
) -> ViolationAgeSummary {
    let mut new_count = 0;
    let mut recent_count = 0;
    let mut chronic_count = 0;

    for v in current_violations {
        let fingerprint = (v.from_module.clone(), v.to_module.clone());
        let age = historical_violation_sets
            .iter()
            .filter(|snap_violations| snap_violations.contains(&fingerprint))
            .count();

        if age == 0 {
            new_count += 1;
        } else if age < 5 {
            recent_count += 1;
        } else {
            chronic_count += 1;
        }
    }

    ViolationAgeSummary {
        new_count,
        recent_count,
        chronic_count,
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

/// A cross-module dependency that violates the declared `depends_on` graph.
#[derive(Debug, Clone)]
pub struct CrossModuleViolation {
    /// Source module config name (e.g., "app").
    pub from_config: String,
    /// Target module config name (e.g., "lib-network").
    pub to_config: String,
    /// Source file path.
    pub from_file: String,
    /// Target file path.
    pub to_file: String,
    /// Line number of the import.
    pub line_number: i32,
}

/// Result of analyzing a monorepo with multiple configured modules.
#[derive(Debug)]
pub struct MonorepoResult {
    /// Per-module audit results: (module_name, audit_result).
    pub module_results: Vec<(String, AuditResult)>,
    /// Cross-module violations (imports not declared in depends_on).
    pub cross_module_violations: Vec<CrossModuleViolation>,
    /// Weighted average score across all modules.
    pub overall_score: f64,
    /// Total source files across all modules.
    pub total_modules: usize,
}

/// Run independent audits per configured module and detect cross-module violations.
pub fn audit_modules(
    all_modules: &[Module],
    all_dependencies: &[Dependency],
    module_configs: &[crate::settings::ModuleConfig],
) -> MonorepoResult {
    let id_to_path: FxHashMap<&str, &str> = all_modules
        .iter()
        .map(|m| (m.id.as_str(), m.path.as_str()))
        .collect();

    // Map each file to which module config it belongs to (first match wins)
    let mut file_to_config: FxHashMap<&str, usize> = FxHashMap::default();
    for module in all_modules {
        for (i, cfg) in module_configs.iter().enumerate() {
            let prefix = format!("{}/", cfg.path);
            if module.path.starts_with(&prefix) || module.path == cfg.path {
                file_to_config.insert(module.id.as_str(), i);
                break;
            }
        }
    }

    // Run independent audit per module
    let mut module_results: Vec<(String, AuditResult)> = Vec::new();
    let mut total_files = 0usize;
    let mut weighted_score_sum = 0.0f64;

    for (i, cfg) in module_configs.iter().enumerate() {
        let module_ids: FxHashSet<&str> = file_to_config
            .iter()
            .filter(|(_, &config_idx)| config_idx == i)
            .map(|(&id, _)| id)
            .collect();

        let filtered_modules: Vec<Module> = all_modules
            .iter()
            .filter(|m| module_ids.contains(m.id.as_str()))
            .cloned()
            .collect();

        let filtered_deps: Vec<Dependency> = all_dependencies
            .iter()
            .filter(|d| {
                module_ids.contains(d.from_module_id.as_str())
                    && module_ids.contains(d.to_module_id.as_str())
            })
            .cloned()
            .collect();

        let result = audit(&filtered_modules, &filtered_deps);
        let file_count = filtered_modules.len();
        weighted_score_sum += result.score * file_count as f64;
        total_files += file_count;
        module_results.push((cfg.name.clone(), result));
    }

    // Detect cross-module violations
    let mut cross_module_violations = Vec::new();
    for dep in all_dependencies {
        let from_cfg = file_to_config.get(dep.from_module_id.as_str()).copied();
        let to_cfg = file_to_config.get(dep.to_module_id.as_str()).copied();

        if let (Some(from_idx), Some(to_idx)) = (from_cfg, to_cfg) {
            if from_idx != to_idx {
                let from_config = &module_configs[from_idx];
                let to_config = &module_configs[to_idx];
                if !from_config.depends_on.contains(&to_config.name) {
                    cross_module_violations.push(CrossModuleViolation {
                        from_config: from_config.name.clone(),
                        to_config: to_config.name.clone(),
                        from_file: id_to_path
                            .get(dep.from_module_id.as_str())
                            .unwrap_or(&"")
                            .to_string(),
                        to_file: id_to_path
                            .get(dep.to_module_id.as_str())
                            .unwrap_or(&"")
                            .to_string(),
                        line_number: dep.line_number,
                    });
                }
            }
        }
    }

    // Apply cross-module penalty to overall score
    let cross_penalty = if total_files > 0 {
        cross_module_violations.len() as f64 / total_files as f64 * 100.0
    } else {
        0.0
    };

    let overall_score = if total_files > 0 {
        (weighted_score_sum / total_files as f64 - cross_penalty).max(0.0)
    } else {
        100.0
    };

    MonorepoResult {
        module_results,
        cross_module_violations,
        overall_score,
        total_modules: total_files,
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

    #[test]
    fn audit_modules_independent_analysis() {
        let modules = vec![
            make_module("a1", "app/src/main.rs"),
            make_module("a2", "app/src/util.rs"),
            make_module("l1", "lib/src/core.rs"),
            make_module("l2", "lib/src/helper.rs"),
        ];
        // Coupling within app module only
        let deps = vec![Dependency {
            from_module_id: "a1".to_string(),
            to_module_id: "a2".to_string(),
            line_number: 1,
        }];
        let configs = vec![
            crate::settings::ModuleConfig {
                name: "app".to_string(),
                path: "app/src".to_string(),
                depends_on: vec![],
            },
            crate::settings::ModuleConfig {
                name: "lib".to_string(),
                path: "lib/src".to_string(),
                depends_on: vec![],
            },
        ];

        let result = audit_modules(&modules, &deps, &configs);
        assert_eq!(result.module_results.len(), 2);
        assert_eq!(result.module_results[0].0, "app");
        assert_eq!(result.module_results[1].0, "lib");
        // lib has no violations
        assert_eq!(result.module_results[1].1.violations.len(), 0);
        assert!(result.cross_module_violations.is_empty());
    }

    #[test]
    fn audit_modules_cross_module_violation() {
        let modules = vec![
            make_module("a1", "app/src/main.rs"),
            make_module("l1", "lib/src/core.rs"),
        ];
        // app imports lib without declaring depends_on
        let deps = vec![Dependency {
            from_module_id: "a1".to_string(),
            to_module_id: "l1".to_string(),
            line_number: 5,
        }];
        let configs = vec![
            crate::settings::ModuleConfig {
                name: "app".to_string(),
                path: "app/src".to_string(),
                depends_on: vec![], // does NOT list "lib"
            },
            crate::settings::ModuleConfig {
                name: "lib".to_string(),
                path: "lib/src".to_string(),
                depends_on: vec![],
            },
        ];

        let result = audit_modules(&modules, &deps, &configs);
        assert_eq!(result.cross_module_violations.len(), 1);
        assert_eq!(result.cross_module_violations[0].from_config, "app");
        assert_eq!(result.cross_module_violations[0].to_config, "lib");
    }

    #[test]
    fn audit_modules_allowed_cross_dep() {
        let modules = vec![
            make_module("a1", "app/src/main.rs"),
            make_module("l1", "lib/src/core.rs"),
        ];
        let deps = vec![Dependency {
            from_module_id: "a1".to_string(),
            to_module_id: "l1".to_string(),
            line_number: 5,
        }];
        let configs = vec![
            crate::settings::ModuleConfig {
                name: "app".to_string(),
                path: "app/src".to_string(),
                depends_on: vec!["lib".to_string()], // allowed
            },
            crate::settings::ModuleConfig {
                name: "lib".to_string(),
                path: "lib/src".to_string(),
                depends_on: vec![],
            },
        ];

        let result = audit_modules(&modules, &deps, &configs);
        assert!(result.cross_module_violations.is_empty());
    }

    #[test]
    fn audit_modules_weighted_score() {
        let modules = vec![
            make_module("a1", "app/src/main.rs"),
            make_module("a2", "app/src/util.rs"),
            make_module("a3", "app/src/helper.rs"),
            make_module("l1", "lib/src/core.rs"),
        ];
        // No deps = perfect scores
        let deps = vec![];
        let configs = vec![
            crate::settings::ModuleConfig {
                name: "app".to_string(),
                path: "app/src".to_string(),
                depends_on: vec![],
            },
            crate::settings::ModuleConfig {
                name: "lib".to_string(),
                path: "lib/src".to_string(),
                depends_on: vec![],
            },
        ];

        let result = audit_modules(&modules, &deps, &configs);
        // Both modules have score 100, weighted average is 100
        assert!((result.overall_score - 100.0).abs() < 0.01);
        assert_eq!(result.total_modules, 4);
    }

    #[test]
    fn audit_modules_empty_config_not_used() {
        // This test verifies the fallback path works by ensuring
        // audit_modules with empty config returns empty results
        let modules = vec![make_module("a1", "src/main.rs")];
        let deps = vec![];
        let configs = vec![];

        let result = audit_modules(&modules, &deps, &configs);
        assert!(result.module_results.is_empty());
        assert_eq!(result.overall_score, 100.0);
    }

    #[test]
    fn independence_fully_internal() {
        let modules = vec![
            make_module("a1", "app/main.rs"),
            make_module("a2", "app/util.rs"),
        ];
        let deps = vec![Dependency {
            from_module_id: "a1".to_string(),
            to_module_id: "a2".to_string(),
            line_number: 1,
        }];
        let result = audit(&modules, &deps);
        let app = result.independence.iter().find(|m| m.dir == "app");
        assert!(app.is_some());
        let app = app.unwrap();
        assert_eq!(app.internal_deps, 1);
        assert_eq!(app.external_deps, 0);
        assert!((app.independence - 1.0).abs() < 0.01);
    }

    #[test]
    fn independence_mixed_deps() {
        let modules = vec![
            make_module("a1", "app/main.rs"),
            make_module("a2", "app/util.rs"),
            make_module("l1", "lib/core.rs"),
        ];
        let deps = vec![
            Dependency {
                from_module_id: "a1".to_string(),
                to_module_id: "a2".to_string(),
                line_number: 1,
            },
            Dependency {
                from_module_id: "a1".to_string(),
                to_module_id: "l1".to_string(),
                line_number: 2,
            },
        ];
        let result = audit(&modules, &deps);
        let app = result.independence.iter().find(|m| m.dir == "app");
        assert!(app.is_some());
        let app = app.unwrap();
        assert_eq!(app.internal_deps, 1);
        assert_eq!(app.external_deps, 1);
        assert!((app.independence - 0.5).abs() < 0.01);
    }

    #[test]
    fn independence_sorted_lowest_first() {
        let modules = vec![
            make_module("a1", "app/main.rs"),
            make_module("a2", "app/util.rs"),
            make_module("l1", "lib/core.rs"),
            make_module("l2", "lib/helper.rs"),
        ];
        let deps = vec![
            Dependency {
                from_module_id: "a1".to_string(),
                to_module_id: "a2".to_string(),
                line_number: 1,
            },
            Dependency {
                from_module_id: "a1".to_string(),
                to_module_id: "l1".to_string(),
                line_number: 2,
            },
            Dependency {
                from_module_id: "l1".to_string(),
                to_module_id: "l2".to_string(),
                line_number: 1,
            },
        ];
        let result = audit(&modules, &deps);
        assert_eq!(result.independence.len(), 2);
        assert_eq!(result.independence[0].dir, "app");
        assert_eq!(result.independence[1].dir, "lib");
    }

    #[test]
    fn instability_pure_consumer() {
        let modules = vec![
            make_module("a1", "src/app/main.rs"),
            make_module("l1", "src/lib/core.rs"),
        ];
        let deps = vec![Dependency {
            from_module_id: "a1".to_string(),
            to_module_id: "l1".to_string(),
            line_number: 1,
        }];
        let result = audit(&modules, &deps);
        let a1 = result
            .hotspots
            .iter()
            .find(|h| h.path == "src/app/main.rs")
            .unwrap();
        assert_eq!(a1.fan_out, 1);
        assert_eq!(a1.fan_in, 0);
        assert!((a1.instability - 1.0).abs() < 0.01);
    }

    #[test]
    fn instability_pure_provider() {
        let modules = vec![
            make_module("a1", "src/app/main.rs"),
            make_module("l1", "src/lib/core.rs"),
        ];
        let deps = vec![Dependency {
            from_module_id: "a1".to_string(),
            to_module_id: "l1".to_string(),
            line_number: 1,
        }];
        let result = audit(&modules, &deps);
        let l1 = result
            .hotspots
            .iter()
            .find(|h| h.path == "src/lib/core.rs")
            .unwrap();
        assert_eq!(l1.fan_in, 1);
        assert_eq!(l1.fan_out, 0);
        assert!((l1.instability - 0.0).abs() < 0.01);
    }

    #[test]
    fn instability_balanced() {
        let modules = vec![
            make_module("a", "src/a.rs"),
            make_module("b", "src/b.rs"),
            make_module("c", "src/c.rs"),
        ];
        let deps = vec![
            Dependency {
                from_module_id: "a".to_string(),
                to_module_id: "b".to_string(),
                line_number: 1,
            },
            Dependency {
                from_module_id: "b".to_string(),
                to_module_id: "c".to_string(),
                line_number: 1,
            },
        ];
        let result = audit(&modules, &deps);
        let b = result
            .hotspots
            .iter()
            .find(|h| h.path == "src/b.rs")
            .unwrap();
        assert_eq!(b.fan_in, 1);
        assert_eq!(b.fan_out, 1);
        assert!((b.instability - 0.5).abs() < 0.01);
    }

    #[test]
    fn blast_radius_leaf_is_zero() {
        let modules = vec![make_module("a", "src/a.rs"), make_module("b", "src/b.rs")];
        let deps = vec![Dependency {
            from_module_id: "a".to_string(),
            to_module_id: "b".to_string(),
            line_number: 1,
        }];
        let result = audit(&modules, &deps);
        let a = result
            .hotspots
            .iter()
            .find(|h| h.path == "src/a.rs")
            .unwrap();
        assert_eq!(a.blast_radius, 0);
    }

    #[test]
    fn blast_radius_direct() {
        let modules = vec![make_module("a", "src/a.rs"), make_module("b", "src/b.rs")];
        let deps = vec![Dependency {
            from_module_id: "a".to_string(),
            to_module_id: "b".to_string(),
            line_number: 1,
        }];
        let result = audit(&modules, &deps);
        let b = result
            .hotspots
            .iter()
            .find(|h| h.path == "src/b.rs")
            .unwrap();
        assert_eq!(b.blast_radius, 1);
    }

    #[test]
    fn blast_radius_transitive() {
        let modules = vec![
            make_module("a", "src/a.rs"),
            make_module("b", "src/b.rs"),
            make_module("c", "src/c.rs"),
        ];
        let deps = vec![
            Dependency {
                from_module_id: "a".to_string(),
                to_module_id: "b".to_string(),
                line_number: 1,
            },
            Dependency {
                from_module_id: "b".to_string(),
                to_module_id: "c".to_string(),
                line_number: 1,
            },
        ];
        let result = audit(&modules, &deps);
        let c = result
            .hotspots
            .iter()
            .find(|h| h.path == "src/c.rs")
            .unwrap();
        assert_eq!(c.blast_radius, 2);
    }

    #[test]
    fn dependency_depth_linear_chain() {
        // a -> b -> c: max depth = 2
        let modules = vec![
            make_module("a", "src/a.rs"),
            make_module("b", "src/b.rs"),
            make_module("c", "src/c.rs"),
        ];
        let deps = vec![
            Dependency {
                from_module_id: "a".to_string(),
                to_module_id: "b".to_string(),
                line_number: 1,
            },
            Dependency {
                from_module_id: "b".to_string(),
                to_module_id: "c".to_string(),
                line_number: 1,
            },
        ];
        let result = audit(&modules, &deps);
        assert_eq!(result.max_depth, 2);
        assert_eq!(result.critical_path.len(), 3);
    }

    #[test]
    fn dependency_depth_no_deps() {
        let modules = vec![make_module("a", "src/a.rs"), make_module("b", "src/b.rs")];
        let result = audit(&modules, &[]);
        assert_eq!(result.max_depth, 0);
    }

    #[test]
    fn violation_age_all_new() {
        let violations = vec![CouplingViolation {
            dir_a: "src/a".to_string(),
            dir_b: "src/b".to_string(),
            from_module: "src/a/main.rs".to_string(),
            to_module: "src/b/lib.rs".to_string(),
            line_number: 1,
            depth: 1,
            weight: 1,
            severity: 0.5,
            is_circular: false,
            cycle_path: Vec::new(),
            cycle_hop_files: Vec::new(),
            cycle_order: 0,
            cycle_hop_counts: Vec::new(),
            weakest_link: None,
            break_cost: 0,
        }];
        // No historical snapshots
        let age = compute_violation_age(&violations, &[]);
        assert_eq!(age.new_count, 1);
        assert_eq!(age.recent_count, 0);
        assert_eq!(age.chronic_count, 0);
    }

    #[test]
    fn violation_age_chronic() {
        let violations = vec![CouplingViolation {
            dir_a: "src/a".to_string(),
            dir_b: "src/b".to_string(),
            from_module: "src/a/main.rs".to_string(),
            to_module: "src/b/lib.rs".to_string(),
            line_number: 1,
            depth: 1,
            weight: 1,
            severity: 0.5,
            is_circular: false,
            cycle_path: Vec::new(),
            cycle_hop_files: Vec::new(),
            cycle_order: 0,
            cycle_hop_counts: Vec::new(),
            weakest_link: None,
            break_cost: 0,
        }];
        // Same violation in 6 historical snapshots -> chronic
        let fp = vec![("src/a/main.rs".to_string(), "src/b/lib.rs".to_string())];
        let historical: Vec<Vec<(String, String)>> = vec![
            fp.clone(),
            fp.clone(),
            fp.clone(),
            fp.clone(),
            fp.clone(),
            fp,
        ];
        let age = compute_violation_age(&violations, &historical);
        assert_eq!(age.new_count, 0);
        assert_eq!(age.chronic_count, 1);
    }
}
