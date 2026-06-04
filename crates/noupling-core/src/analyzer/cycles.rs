//! Circular dependency detection.
//!
//! Uses Tarjan's strongly-connected-components algorithm to find cycles in
//! the sibling-directory dependency graph, then enumerates the mutual pairs
//! (or a representative directed cycle) within each non-trivial SCC.

use fxhash::{FxHashMap, FxHashSet};

use crate::core::Dependency;

/// A detected cycle at the directory level with the full path of directories involved.
pub(super) struct DetectedCycle {
    /// Full cycle path as directory paths (e.g., ["src/main", "src/analytics", "src/deeplink", "src/main"])
    pub dir_path: Vec<String>,
    /// For each hop in the cycle, the file that causes the dependency (from_file -> to_file)
    /// Length is dir_path.len() - 1 (one edge per hop)
    pub hop_files: Vec<(String, String, i32)>,
    /// For each hop, how many imports cross that edge. Used to find the weakest link.
    pub hop_import_counts: Vec<usize>,
}

/// Detect circular dependencies among sibling directories using their D_acc sets.
/// A cycle exists when sibling A's D_acc points into B, B's into C, ..., and back to A.
pub(super) fn detect_sibling_cycles(
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

    // For each SCC of size >= 2, enumerate every mutual-dependency pair
    // (i, j) where both i -> j and j -> i exist. Each pair is reported as
    // a separate 2-cycle violation. This gives users actionable visibility
    // into specific module pairs that need decoupling, while still being
    // bounded (O(edges in SCC) instead of O(elementary cycles)).
    let mut cycles: Vec<DetectedCycle> = Vec::new();
    let mut reported: FxHashSet<(usize, usize)> = FxHashSet::default();

    for scc in sccs {
        if scc.len() < 2 {
            continue;
        }
        let scc_set: FxHashSet<usize> = scc.iter().copied().collect();

        // Find all mutual pairs within the SCC
        for &i in &scc {
            if let Some(neighbors) = adj.get(&i) {
                for &j in neighbors {
                    if !scc_set.contains(&j) || i == j {
                        continue;
                    }
                    // Check reverse edge exists
                    let reverse = adj.get(&j).is_some_and(|nbrs| nbrs.contains(&i));
                    if !reverse {
                        continue;
                    }
                    // Canonical ordering to dedupe
                    let key = if i < j { (i, j) } else { (j, i) };
                    if !reported.insert(key) {
                        continue;
                    }
                    let (a, b) = key;
                    let dir_path = vec![
                        siblings[a].clone(),
                        siblings[b].clone(),
                        siblings[a].clone(),
                    ];
                    let hop_files = vec![
                        edge_files.get(&(a, b)).cloned().unwrap_or_default(),
                        edge_files.get(&(b, a)).cloned().unwrap_or_default(),
                    ];
                    let hop_import_counts = vec![
                        edge_import_count.get(&(a, b)).copied().unwrap_or(1),
                        edge_import_count.get(&(b, a)).copied().unwrap_or(1),
                    ];
                    cycles.push(DetectedCycle {
                        dir_path,
                        hop_files,
                        hop_import_counts,
                    });
                }
            }
        }

        // If no mutual pairs were found in this SCC (e.g. A→B→C→A with no
        // reverse edges), find a directed cycle through the SCC so the user
        // still sees the violation.
        if reported.is_empty()
            || scc.iter().all(|n| {
                !scc.iter().any(|m| {
                    n != m && {
                        let key = if n < m { (*n, *m) } else { (*m, *n) };
                        reported.contains(&key)
                    }
                })
            })
        {
            // Check if we already reported anything for this SCC
            let scc_covered: bool = scc.iter().any(|n| {
                scc.iter().any(|m| {
                    if n >= m {
                        return false;
                    }
                    reported.contains(&(*n, *m))
                })
            });
            if !scc_covered {
                // DFS from first node to find a cycle within the SCC
                if let Some(cycle_nodes) = find_cycle_in_scc(&scc_set, &adj, scc[0]) {
                    let mut dir_path: Vec<String> = cycle_nodes
                        .iter()
                        .map(|&idx| siblings[idx].clone())
                        .collect();
                    dir_path.push(siblings[cycle_nodes[0]].clone()); // close the loop

                    let mut hop_files_list = Vec::new();
                    let mut hop_counts = Vec::new();
                    for w in cycle_nodes.windows(2) {
                        hop_files_list
                            .push(edge_files.get(&(w[0], w[1])).cloned().unwrap_or_default());
                        hop_counts.push(edge_import_count.get(&(w[0], w[1])).copied().unwrap_or(1));
                    }
                    // Closing edge
                    let last = *cycle_nodes.last().unwrap();
                    let first = cycle_nodes[0];
                    hop_files_list
                        .push(edge_files.get(&(last, first)).cloned().unwrap_or_default());
                    hop_counts.push(edge_import_count.get(&(last, first)).copied().unwrap_or(1));

                    cycles.push(DetectedCycle {
                        dir_path,
                        hop_files: hop_files_list,
                        hop_import_counts: hop_counts,
                    });
                }
            }
        }
    }

    cycles
}

/// Find a simple directed cycle within an SCC starting from `start`.
/// Returns the cycle as a list of node indices (without the closing duplicate).
fn find_cycle_in_scc(
    scc_set: &FxHashSet<usize>,
    adj: &FxHashMap<usize, Vec<usize>>,
    start: usize,
) -> Option<Vec<usize>> {
    let mut visited: FxHashMap<usize, usize> = FxHashMap::default(); // node -> parent
    let mut stack = vec![(start, 0usize)]; // (node, neighbor_index)
    visited.insert(start, usize::MAX);

    while let Some((node, ni)) = stack.last_mut() {
        let node = *node;
        let neighbors = adj.get(&node);
        let len = neighbors.map(|n| n.len()).unwrap_or(0);
        if *ni >= len {
            stack.pop();
            continue;
        }
        let next = neighbors.unwrap()[*ni];
        *ni += 1;
        if !scc_set.contains(&next) {
            continue;
        }
        if next == start {
            // Found cycle back to start — extract path
            let path: Vec<usize> = stack.iter().map(|(n, _)| *n).collect();
            return Some(path);
        }
        if let std::collections::hash_map::Entry::Vacant(e) = visited.entry(next) {
            e.insert(node);
            stack.push((next, 0));
        }
    }
    None
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
