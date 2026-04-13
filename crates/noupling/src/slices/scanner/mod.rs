mod discovery;
mod parser;
mod resolver;

pub use discovery::discover_files;
pub use parser::parse_rust_imports;
pub use parser::ImportEntry;
pub use resolver::resolve_import;

use crate::core::{Dependency, Node};
use anyhow::Result;
use rayon::prelude::*;
use std::path::Path;

pub struct ScanResult {
    pub nodes: Vec<Node>,
    pub dependencies: Vec<Dependency>,
}

pub fn scan_project(root: &Path, snapshot_id: &str) -> Result<ScanResult> {
    let root = root.canonicalize()?;
    let nodes = discover_files(&root, snapshot_id)?;

    let file_nodes: Vec<&Node> = nodes
        .iter()
        .filter(|n| matches!(n.node_type, crate::core::NodeType::File))
        .collect();

    let all_paths: Vec<String> = nodes.iter().map(|n| n.path.clone()).collect();

    let dependencies: Vec<Dependency> = file_nodes
        .par_iter()
        .filter_map(|node| {
            let path = Path::new(&node.path);
            if path.extension().and_then(|e| e.to_str()) != Some("rs") {
                return None;
            }
            let source = std::fs::read_to_string(path).ok()?;
            let imports = parse_rust_imports(&source);
            let deps: Vec<Dependency> = imports
                .iter()
                .filter_map(|entry| {
                    let resolved = resolve_import(&entry.path, &node.path, &root, &all_paths)?;
                    let to_node = nodes.iter().find(|n| n.path == resolved)?;
                    Some(Dependency {
                        from_node_id: node.id.clone(),
                        to_node_id: to_node.id.clone(),
                        line_number: entry.line_number,
                    })
                })
                .collect();
            Some(deps)
        })
        .flatten()
        .collect();

    Ok(ScanResult {
        nodes,
        dependencies,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::NodeType;

    #[test]
    fn scan_project_discovers_nodes_and_deps() {
        let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/mock_rust_project");
        let result = scan_project(&fixture, "test-snap").unwrap();

        // Should have dirs and files
        let dirs: Vec<&Node> = result
            .nodes
            .iter()
            .filter(|n| matches!(n.node_type, NodeType::Dir))
            .collect();
        let files: Vec<&Node> = result
            .nodes
            .iter()
            .filter(|n| matches!(n.node_type, NodeType::File))
            .collect();

        // root, src, modules = 3 dirs
        assert_eq!(dirs.len(), 3, "dirs: {:?}", dirs.iter().map(|n| &n.name).collect::<Vec<_>>());
        // main.rs, mod.rs, helper.rs = 3 files
        assert_eq!(files.len(), 3, "files: {:?}", files.iter().map(|n| &n.name).collect::<Vec<_>>());

        // main.rs has `use crate::modules::helper` which should resolve to helper.rs
        assert!(
            !result.dependencies.is_empty(),
            "Expected at least one dependency from main.rs -> helper.rs"
        );
    }

    #[test]
    fn scan_project_parallel_produces_consistent_results() {
        let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/mock_rust_project");

        // Run twice and verify consistent node/dep counts
        let r1 = scan_project(&fixture, "snap-1").unwrap();
        let r2 = scan_project(&fixture, "snap-2").unwrap();

        assert_eq!(r1.nodes.len(), r2.nodes.len());
        assert_eq!(r1.dependencies.len(), r2.dependencies.len());
    }
}
