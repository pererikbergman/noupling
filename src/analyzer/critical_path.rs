//! Dependency depth + critical path computation.
//!
//! Finds the longest dependency chain in the module graph and reconstructs
//! the path of file paths from root to deepest leaf.

use fxhash::FxHashMap;

use crate::core::{Dependency, Module};

/// Compute the maximum dependency depth and reconstruct the longest path.
/// Returns `(max_depth, critical_path)` where `critical_path` is a list of
/// file paths from root to deepest leaf. For nodes that are part of cycles
/// in the dependency graph, depth stays 0.
pub fn compute_critical_path(
    modules: &[Module],
    dependencies: &[Dependency],
) -> (usize, Vec<String>) {
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

    (max_depth, critical_path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_module(id: &str, path: &str) -> Module {
        use crate::core::ModuleType;
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
        let (max_depth, critical_path) = compute_critical_path(&modules, &deps);
        assert_eq!(max_depth, 2);
        assert_eq!(critical_path.len(), 3);
    }

    #[test]
    fn dependency_depth_no_deps() {
        let modules = vec![make_module("a", "src/a.rs"), make_module("b", "src/b.rs")];
        let (max_depth, _) = compute_critical_path(&modules, &[]);
        assert_eq!(max_depth, 0);
    }
}
