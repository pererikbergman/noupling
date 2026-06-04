//! Per-directory cohesion metrics: ratio of cross-logical-node dependencies
//! to the maximum possible logical-node pairs in the directory.
//!
//! See `docs/dependency-graph.md` § Analysis Step 2 for the architectural model.

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

/// One "logical node" of a directory — either a direct file (identified by its
/// module id) or a direct subdirectory (identified by its name relative to the
/// parent). Two `LogicalNode`s are equal iff they refer to the same logical
/// child of the same parent.
///
/// In the cohesion calculation, edges that cross between different
/// `LogicalNode`s count; edges between files mapped to the same `LogicalNode`
/// (e.g. two files inside the same direct subdirectory) do not.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(super) enum LogicalNode {
    /// A direct file in the parent directory. Carries the file's module id.
    File(String),
    /// A direct subdirectory of the parent directory, identified by its
    /// single-component name (e.g. "scanner" — not "src/scanner").
    Subdir(String),
}

/// Per-directory index mapping each directory to its logical-node children
/// and its kind. Built once from the module list and queried throughout the
/// cohesion calculation. See `docs/dependency-graph.md` § Analysis Step 2.
///
/// This is the *deep module* the PRD called for: it encapsulates the entire
/// "what are this directory's logical children, and which logical node does
/// any given file under this directory belong to?" question behind a small
/// interface. Future analyses (D_acc aggregation, coupling-between-subdirs)
/// can lean on the same primitive without re-implementing the grouping logic.
pub(super) struct LogicalNodeIndex {
    /// For each directory in the project, the set of its direct logical-node
    /// children — direct files (as `File(id)`) plus immediate subdirectories
    /// (as `Subdir(name)`). Containers have only `Subdir(_)` entries.
    children: FxHashMap<String, Vec<LogicalNode>>,
    /// For each directory, its kind (`Container` or `Package`). Derived from
    /// whether any `LogicalNode::File(_)` appears in its children.
    kinds: FxHashMap<String, DirectoryKind>,
    /// Map from module id → its file path. Needed to translate a `Dependency`'s
    /// `from_module_id` / `to_module_id` into paths during cohesion math.
    id_to_path: FxHashMap<String, String>,
}

impl LogicalNodeIndex {
    pub(super) fn build(modules: &[Module]) -> Self {
        // id → path
        let id_to_path: FxHashMap<String, String> = modules
            .iter()
            .map(|m| (m.id.clone(), m.path.clone()))
            .collect();

        // Collect every directory that exists in the project tree (every
        // ancestor of every file). Containers won't have direct files but
        // must still appear as keys in `children` / `kinds`.
        let mut all_dirs: FxHashSet<String> = FxHashSet::default();
        for m in modules {
            let mut current = std::path::Path::new(&m.path).parent();
            while let Some(d) = current {
                let s = d.to_string_lossy().to_string();
                if s.is_empty() {
                    break;
                }
                all_dirs.insert(s);
                current = d.parent();
            }
        }

        // Build `children` for each directory: every direct file is a `File`
        // logical node; every immediate subdirectory is a `Subdir` logical
        // node (deduped by name).
        let mut children: FxHashMap<String, Vec<LogicalNode>> = FxHashMap::default();
        let mut subdir_seen: FxHashMap<String, FxHashSet<String>> = FxHashMap::default();
        for dir in &all_dirs {
            children.entry(dir.clone()).or_default();
            subdir_seen.entry(dir.clone()).or_default();
        }

        for m in modules {
            // The module is a direct file of its parent directory.
            let parent = std::path::Path::new(&m.path)
                .parent()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_default();
            if !parent.is_empty() {
                children
                    .entry(parent)
                    .or_default()
                    .push(LogicalNode::File(m.id.clone()));
            }

            // For every ancestor *above* the parent, the path component
            // immediately under that ancestor is a Subdir logical node of it.
            let mut walker = std::path::Path::new(&m.path).parent(); // file's parent
            while let Some(d) = walker {
                if let Some(grandparent) = d.parent() {
                    let gp_str = grandparent.to_string_lossy().to_string();
                    if gp_str.is_empty() {
                        break;
                    }
                    let subdir_name = d
                        .file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_default();
                    if !subdir_name.is_empty() {
                        let seen = subdir_seen.entry(gp_str.clone()).or_default();
                        if seen.insert(subdir_name.clone()) {
                            children
                                .entry(gp_str)
                                .or_default()
                                .push(LogicalNode::Subdir(subdir_name));
                        }
                    }
                    walker = grandparent.parent();
                } else {
                    break;
                }
            }
        }

        // Kind: Package iff any child is a `File`.
        let mut kinds: FxHashMap<String, DirectoryKind> = FxHashMap::default();
        for (dir, cs) in &children {
            let has_file = cs.iter().any(|c| matches!(c, LogicalNode::File(_)));
            kinds.insert(
                dir.clone(),
                if has_file {
                    DirectoryKind::Package
                } else {
                    DirectoryKind::Container
                },
            );
        }

        LogicalNodeIndex {
            children,
            kinds,
            id_to_path,
        }
    }

    pub(super) fn kind(&self, dir: &str) -> Option<DirectoryKind> {
        self.kinds.get(dir).copied()
    }

    pub(super) fn children(&self, dir: &str) -> &[LogicalNode] {
        self.children.get(dir).map(|v| v.as_slice()).unwrap_or(&[])
    }

    pub(super) fn dirs(&self) -> impl Iterator<Item = &String> {
        self.kinds.keys()
    }

    /// Given a file (by its module id) and a directory `dir` that contains the
    /// file somewhere in its subtree, return the `LogicalNode` under `dir` to
    /// which the file belongs.
    ///
    /// - If the file is a direct child of `dir`, returns `Some(File(id))`.
    /// - If the file is deeper (inside a subdirectory of `dir`), returns
    ///   `Some(Subdir(name))` where `name` is the path component immediately
    ///   under `dir`.
    /// - Returns `None` if the file is not under `dir` at all (different
    ///   subtree), or if the file's path can't be looked up.
    pub(super) fn logical_node_of(&self, file_id: &str, dir: &str) -> Option<LogicalNode> {
        let path = self.id_to_path.get(file_id)?;
        let dir_prefix = format!("{}/", dir);
        if !path.starts_with(&dir_prefix) {
            return None;
        }
        let suffix = &path[dir_prefix.len()..];
        // If the suffix has no '/', the file is directly in `dir`.
        match suffix.find('/') {
            None => Some(LogicalNode::File(file_id.to_string())),
            Some(slash_idx) => Some(LogicalNode::Subdir(suffix[..slash_idx].to_string())),
        }
    }
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

/// Compute per-directory cohesion for every directory in the project, using
/// the logical-node rule documented in `docs/dependency-graph.md` § Analysis
/// Step 2.
///
/// For each directory `X`:
/// - **Container** (no direct files, only subdirectories) → `cohesion: None`.
/// - **Package** (≥1 direct file) → treat each direct child (file or
///   subdirectory) as one logical node. `n = |children|`, `pairs = n × (n−1)`,
///   `internal = count of edges crossing different logical nodes`. Cohesion =
///   `internal / pairs` (or `Some(0.0)` when `pairs == 0`).
///
/// Subdirectories are opaque: edges fully inside one subdirectory don't
/// contribute to the parent's cohesion (they contribute to that subdirectory's
/// own cohesion at its own ply).
///
/// Both Containers and Packages appear in the result. Output is sorted with
/// Packages first (by cohesion ascending), then Containers (alphabetically).
pub fn compute_cohesion(modules: &[Module], dependencies: &[Dependency]) -> Vec<CohesionMetrics> {
    let index = LogicalNodeIndex::build(modules);

    let mut cohesion: Vec<CohesionMetrics> = index
        .dirs()
        .map(|dir| {
            let kind = index.kind(dir).unwrap_or(DirectoryKind::Container);
            let kids = index.children(dir);
            let n = kids.len();

            match kind {
                DirectoryKind::Container => CohesionMetrics {
                    dir: dir.clone(),
                    kind,
                    n_children: n,
                    internal_deps: 0,
                    cohesion: None,
                },
                DirectoryKind::Package => {
                    // Count edges where both endpoints are under `dir` but
                    // belong to *different* logical-node children of `dir`.
                    let internal_deps = dependencies
                        .iter()
                        .filter(|d| {
                            let from_node = index.logical_node_of(&d.from_module_id, dir);
                            let to_node = index.logical_node_of(&d.to_module_id, dir);
                            match (from_node, to_node) {
                                (Some(a), Some(b)) => a != b,
                                _ => false,
                            }
                        })
                        .count();
                    let pairs = n * n.saturating_sub(1);
                    let score = if pairs > 0 {
                        internal_deps as f64 / pairs as f64
                    } else {
                        0.0
                    };
                    CohesionMetrics {
                        dir: dir.clone(),
                        kind,
                        n_children: n,
                        internal_deps,
                        cohesion: Some(score),
                    }
                }
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

    // ── LogicalNodeIndex unit tests ──

    #[test]
    fn index_classifies_container_directory() {
        let modules = vec![
            file_module("a", "src/features/auth/login.rs"),
            file_module("b", "src/features/billing/invoice.rs"),
        ];
        let index = LogicalNodeIndex::build(&modules);
        assert_eq!(index.kind("src/features"), Some(DirectoryKind::Container));
    }

    #[test]
    fn index_classifies_package_directory() {
        let modules = vec![file_module("a", "src/utils/lone.rs")];
        let index = LogicalNodeIndex::build(&modules);
        assert_eq!(index.kind("src/utils"), Some(DirectoryKind::Package));
    }

    #[test]
    fn index_children_of_mixed_package_includes_files_and_subdirs() {
        // src/{main.rs, scanner/x.rs} → 2 logical nodes: File(main), Subdir("scanner")
        let modules = vec![
            file_module("main", "src/main.rs"),
            file_module("x", "src/scanner/x.rs"),
        ];
        let index = LogicalNodeIndex::build(&modules);
        let kids = index.children("src");
        assert_eq!(kids.len(), 2);
        assert!(kids.contains(&LogicalNode::File("main".into())));
        assert!(kids.contains(&LogicalNode::Subdir("scanner".into())));
    }

    #[test]
    fn index_logical_node_of_returns_file_for_direct_child() {
        let modules = vec![file_module("main", "src/main.rs")];
        let index = LogicalNodeIndex::build(&modules);
        assert_eq!(
            index.logical_node_of("main", "src"),
            Some(LogicalNode::File("main".into()))
        );
    }

    #[test]
    fn index_logical_node_of_returns_subdir_for_nested_file() {
        // A file deep inside scanner/parsers/python.rs, asked at the src/ level,
        // should report Subdir("scanner") — the immediate path component under src.
        let modules = vec![file_module("py", "src/scanner/parsers/python.rs")];
        let index = LogicalNodeIndex::build(&modules);
        assert_eq!(
            index.logical_node_of("py", "src"),
            Some(LogicalNode::Subdir("scanner".into()))
        );
    }

    #[test]
    fn index_logical_node_of_returns_none_when_file_not_under_dir() {
        // Asking for a file that lives in a different subtree.
        let modules = vec![
            file_module("a", "src/scanner/foo.rs"),
            file_module("b", "src/storage/bar.rs"),
        ];
        let index = LogicalNodeIndex::build(&modules);
        // file "a" (src/scanner/foo.rs) is not under src/storage
        assert_eq!(index.logical_node_of("a", "src/storage"), None);
    }

    // ── compute_cohesion behaviour tests ──

    fn dep(from: &str, to: &str) -> Dependency {
        Dependency {
            from_module_id: from.into(),
            to_module_id: to.into(),
            line_number: 1,
        }
    }

    #[test]
    fn worked_example_from_doc_yields_four_sixths() {
        // From docs/dependency-graph.md § Analysis Step 2:
        //   src/ has direct file main.rs + subdirs scanner/ and storage/.
        //   children(src) = { main.rs, scanner, storage }  → n = 3, pairs = 6
        //
        // Edges in E:
        //   main.rs → scanner/foo.rs     (across: main → scanner)         ✓
        //   scanner/x.rs → scanner/y.rs  (inside scanner, opaque)         ✗
        //   scanner/x.rs → storage/y.rs  (across: scanner → storage)      ✓
        //   scanner/x.rs → storage/z.rs  (across: scanner → storage, ‖)   ✓
        //   main.rs → storage/q.rs       (across: main → storage)         ✓
        //
        // internal(src) = 4 → cohesion(src) = 4/6 ≈ 0.667
        let modules = vec![
            file_module("main", "src/main.rs"),
            file_module("foo", "src/scanner/foo.rs"),
            file_module("x", "src/scanner/x.rs"),
            file_module("y", "src/scanner/y.rs"),
            file_module("sy", "src/storage/y.rs"),
            file_module("sz", "src/storage/z.rs"),
            file_module("sq", "src/storage/q.rs"),
        ];
        let deps = vec![
            dep("main", "foo"),
            dep("x", "y"),
            dep("x", "sy"),
            dep("x", "sz"),
            dep("main", "sq"),
        ];

        let result = compute_cohesion(&modules, &deps);

        let src = result.iter().find(|c| c.dir == "src").expect("src");
        assert_eq!(src.kind, DirectoryKind::Package);
        assert_eq!(src.n_children, 3, "main.rs + scanner + storage");
        assert_eq!(src.internal_deps, 4, "4 across-logical-node edges");
        let val = src.cohesion.unwrap();
        assert!(
            (val - 4.0 / 6.0).abs() < 1e-9,
            "expected ~0.667, got {}",
            val
        );
    }

    #[test]
    fn edges_fully_inside_a_subdirectory_do_not_contribute_to_parent_cohesion() {
        // src/ has one direct file (main.rs) and one subdirectory (scanner/)
        // containing two files. The edge scanner/a.rs → scanner/b.rs lives
        // entirely inside scanner — it must NOT show up in src/'s cohesion.
        let modules = vec![
            file_module("main", "src/main.rs"),
            file_module("a", "src/scanner/a.rs"),
            file_module("b", "src/scanner/b.rs"),
        ];
        let deps = vec![dep("a", "b")]; // inside scanner, opaque at src level

        let result = compute_cohesion(&modules, &deps);

        let src = result.iter().find(|c| c.dir == "src").expect("src");
        assert_eq!(src.kind, DirectoryKind::Package);
        assert_eq!(src.n_children, 2, "main.rs + scanner subdir");
        assert_eq!(
            src.internal_deps, 0,
            "the scanner-internal edge must NOT count at the src level"
        );
        assert_eq!(src.cohesion, Some(0.0));

        // Verify the same edge DOES count for scanner/'s own cohesion (the
        // opaque-from-outside rule does not hide it from the inside).
        let scanner = result
            .iter()
            .find(|c| c.dir == "src/scanner")
            .expect("src/scanner");
        assert_eq!(scanner.internal_deps, 1);
    }

    #[test]
    fn parallel_edges_between_logical_nodes_count_multiple_times() {
        // src/{main.rs, scanner/x.rs}: main → x on three different lines.
        // n = 2, pairs = 2, internal = 3 → cohesion = 1.5 (allowed to exceed 1.0)
        let modules = vec![
            file_module("main", "src/main.rs"),
            file_module("x", "src/scanner/x.rs"),
        ];
        let deps = vec![
            Dependency {
                from_module_id: "main".into(),
                to_module_id: "x".into(),
                line_number: 3,
            },
            Dependency {
                from_module_id: "main".into(),
                to_module_id: "x".into(),
                line_number: 7,
            },
            Dependency {
                from_module_id: "main".into(),
                to_module_id: "x".into(),
                line_number: 17,
            },
        ];

        let result = compute_cohesion(&modules, &deps);
        let src = result.iter().find(|c| c.dir == "src").expect("src");
        assert_eq!(src.internal_deps, 3);
        assert_eq!(src.n_children, 2);
        assert_eq!(src.cohesion, Some(1.5));
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
