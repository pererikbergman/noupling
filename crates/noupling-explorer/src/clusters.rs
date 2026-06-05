//! Module clustering for the Force view (#278 follow-up).
//!
//! Given the dependency graph, partition the nodes into communities
//! so the Force view can render cluster boundaries around tightly
//! coupled groups.
//!
//! We use **label propagation** rather than Louvain proper: it's
//! O(iterations × |E|), converges fast on typical codebases, and
//! produces communities aligned with user intuition (tightly imported
//! groups end up together). Louvain proper would maximize modularity
//! but requires a non-trivial implementation; LPA is a recognized
//! community-detection algorithm (Raghavan, Albert, Kumara 2007) and
//! good enough for visual cluster hints in the Explorer.
//!
//! The output is a list of cluster entries, each carrying the node
//! ids that belong to it. The Explorer's Force view renders a faint
//! convex hull behind each cluster's nodes.

use noupling_core::core::{Dependency, Module};
use std::collections::HashMap;

const MAX_ITERATIONS: usize = 8;

/// A single detected cluster — just node ids; the Force view computes
/// the visual hull at render time from the layout positions.
#[derive(Debug, Clone)]
pub struct ClusterEntry {
    pub id: String,
    pub members: Vec<String>,
}

/// Detect clusters across the codebase using label propagation on the
/// container/package dependency graph. Files are summarised up to
/// their nearest container so the cluster ids stay readable.
pub fn detect_clusters(modules: &[Module], dependencies: &[Dependency]) -> Vec<ClusterEntry> {
    // Build a node set from container/package paths. Files contribute
    // through their parent directory.
    let mut node_indices: HashMap<String, usize> = HashMap::new();
    let mut nodes: Vec<String> = Vec::new();

    let containerise = |path: &str| -> String {
        match path.rfind('/') {
            Some(i) => path[..i].to_string(),
            None => path.to_string(),
        }
    };

    for m in modules {
        let key = containerise(&m.path);
        if !node_indices.contains_key(&key) {
            node_indices.insert(key.clone(), nodes.len());
            nodes.push(key);
        }
    }
    if nodes.is_empty() {
        return Vec::new();
    }

    // Build an adjacency list from container-to-container edges.
    let id_to_path: HashMap<&str, &str> = modules
        .iter()
        .map(|m| (m.id.as_str(), m.path.as_str()))
        .collect();
    let mut adjacency: Vec<Vec<usize>> = vec![Vec::new(); nodes.len()];
    for d in dependencies {
        let from_path = match id_to_path.get(d.from_module_id.as_str()) {
            Some(p) => *p,
            None => continue,
        };
        let to_path = match id_to_path.get(d.to_module_id.as_str()) {
            Some(p) => *p,
            None => continue,
        };
        let a_key = containerise(from_path);
        let b_key = containerise(to_path);
        if a_key == b_key {
            continue;
        }
        if let (Some(&a), Some(&b)) = (node_indices.get(&a_key), node_indices.get(&b_key)) {
            adjacency[a].push(b);
            adjacency[b].push(a);
        }
    }

    // Label propagation: each node starts in its own community, then
    // iteratively adopts the most-common label among its neighbours.
    let mut labels: Vec<usize> = (0..nodes.len()).collect();
    let mut changed = true;
    let mut iter = 0;
    while changed && iter < MAX_ITERATIONS {
        changed = false;
        iter += 1;
        for v in 0..nodes.len() {
            if adjacency[v].is_empty() {
                continue;
            }
            let mut counts: HashMap<usize, usize> = HashMap::new();
            for &u in &adjacency[v] {
                *counts.entry(labels[u]).or_insert(0) += 1;
            }
            // Choose the label with the highest count; ties broken by
            // smaller label id for determinism. Skip if no change.
            let best = counts
                .into_iter()
                .max_by(|a, b| a.1.cmp(&b.1).then(b.0.cmp(&a.0)))
                .map(|(label, _)| label);
            if let Some(b) = best {
                if labels[v] != b {
                    labels[v] = b;
                    changed = true;
                }
            }
        }
    }

    // Bucket nodes by final label.
    let mut buckets: HashMap<usize, Vec<String>> = HashMap::new();
    for (idx, &label) in labels.iter().enumerate() {
        buckets.entry(label).or_default().push(nodes[idx].clone());
    }

    // Skip singleton clusters — they don't help the visual story and
    // can dwarf the genuine groups.
    let mut entries: Vec<ClusterEntry> = buckets
        .into_iter()
        .filter(|(_, members)| members.len() >= 2)
        .enumerate()
        .map(|(i, (_, members))| ClusterEntry {
            id: format!("cluster-{}", i + 1),
            members,
        })
        .collect();
    entries.sort_by_key(|c| std::cmp::Reverse(c.members.len()));
    entries
}

#[cfg(test)]
mod tests {
    use super::*;
    use noupling_core::core::{Module, ModuleType};

    fn mk_module(id: &str, path: &str) -> Module {
        Module {
            id: id.into(),
            snapshot_id: "s".into(),
            parent_id: None,
            name: path.split('/').next_back().unwrap_or(path).into(),
            path: path.into(),
            module_type: ModuleType::File,
            depth: path.matches('/').count() as i32,
        }
    }

    fn mk_dep(from: &str, to: &str) -> Dependency {
        Dependency {
            from_module_id: from.into(),
            to_module_id: to.into(),
            line_number: 1,
        }
    }

    #[test]
    fn isolated_packages_produce_no_clusters() {
        // Two disjoint files in two different packages — no edges
        // means no community grouping, every node stays solo and
        // singletons get filtered out.
        let modules = vec![
            mk_module("a", "src/foo/a.rs"),
            mk_module("b", "src/bar/b.rs"),
        ];
        let clusters = detect_clusters(&modules, &[]);
        assert!(clusters.is_empty());
    }

    #[test]
    fn tightly_coupled_pair_groups_into_one_cluster() {
        let modules = vec![
            mk_module("a", "src/foo/a.rs"),
            mk_module("b", "src/bar/b.rs"),
            mk_module("c", "src/baz/c.rs"),
        ];
        let deps = vec![mk_dep("a", "b"), mk_dep("b", "a")];
        let clusters = detect_clusters(&modules, &deps);
        // src/foo and src/bar should land together; src/baz stays
        // singleton (filtered out).
        assert_eq!(clusters.len(), 1);
        let mut members = clusters[0].members.clone();
        members.sort();
        assert_eq!(members, vec!["src/bar".to_string(), "src/foo".to_string()]);
    }
}
