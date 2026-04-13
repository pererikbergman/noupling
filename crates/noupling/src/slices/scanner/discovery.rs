use anyhow::Result;
use std::path::Path;
use uuid::Uuid;

use crate::core::{Node, NodeType};

const IGNORED_DIRS: &[&str] = &[".git", "target", "node_modules", ".noupling", ".agent"];

pub fn discover_files(root: &Path, snapshot_id: &str) -> Result<Vec<Node>> {
    let mut nodes = Vec::new();
    let root_canonical = root.canonicalize()?;
    walk_directory(&root_canonical, snapshot_id, None, 0, &mut nodes)?;
    Ok(nodes)
}

fn walk_directory(
    dir: &Path,
    snapshot_id: &str,
    parent_id: Option<&str>,
    depth: i32,
    nodes: &mut Vec<Node>,
) -> Result<()> {
    let dir_name = dir
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();

    let dir_id = Uuid::new_v4().to_string();

    nodes.push(Node {
        id: dir_id.clone(),
        snapshot_id: snapshot_id.to_string(),
        parent_id: parent_id.map(|s| s.to_string()),
        name: dir_name,
        path: dir.to_string_lossy().to_string(),
        node_type: NodeType::Dir,
        depth,
    });

    let mut entries: Vec<_> = std::fs::read_dir(dir)?
        .filter_map(|e| e.ok())
        .collect();
    entries.sort_by_key(|e| e.file_name());

    for entry in entries {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();

        if path.is_dir() {
            if IGNORED_DIRS.contains(&name.as_str()) {
                continue;
            }
            walk_directory(&path, snapshot_id, Some(&dir_id), depth + 1, nodes)?;
        } else {
            let file_id = Uuid::new_v4().to_string();
            nodes.push(Node {
                id: file_id,
                snapshot_id: snapshot_id.to_string(),
                parent_id: Some(dir_id.clone()),
                name,
                path: path.to_string_lossy().to_string(),
                node_type: NodeType::File,
                depth: depth + 1,
            });
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn create_mock_project(dir: &Path) {
        let src = dir.join("src");
        fs::create_dir_all(&src).unwrap();
        fs::write(src.join("main.rs"), "fn main() {}").unwrap();
        fs::write(src.join("lib.rs"), "pub mod utils;").unwrap();

        let utils = src.join("utils");
        fs::create_dir_all(&utils).unwrap();
        fs::write(utils.join("mod.rs"), "").unwrap();

        // Should be ignored
        let git = dir.join(".git");
        fs::create_dir_all(&git).unwrap();
        fs::write(git.join("HEAD"), "ref: refs/heads/main").unwrap();
    }

    #[test]
    fn discovers_all_files_and_dirs() {
        let tmp = tempfile::tempdir().unwrap();
        create_mock_project(tmp.path());

        let nodes = discover_files(tmp.path(), "snap-1").unwrap();

        let dirs: Vec<&Node> = nodes
            .iter()
            .filter(|n| matches!(n.node_type, NodeType::Dir))
            .collect();
        let files: Vec<&Node> = nodes
            .iter()
            .filter(|n| matches!(n.node_type, NodeType::File))
            .collect();

        // root, src, utils (not .git)
        assert_eq!(dirs.len(), 3, "Expected 3 dirs, got: {:?}", dirs.iter().map(|n| &n.name).collect::<Vec<_>>());
        // main.rs, lib.rs, mod.rs
        assert_eq!(files.len(), 3, "Expected 3 files, got: {:?}", files.iter().map(|n| &n.name).collect::<Vec<_>>());
    }

    #[test]
    fn root_node_has_no_parent() {
        let tmp = tempfile::tempdir().unwrap();
        create_mock_project(tmp.path());
        let nodes = discover_files(tmp.path(), "snap-1").unwrap();
        let root = &nodes[0];
        assert!(root.parent_id.is_none());
        assert_eq!(root.depth, 0);
    }

    #[test]
    fn child_nodes_reference_parent() {
        let tmp = tempfile::tempdir().unwrap();
        create_mock_project(tmp.path());
        let nodes = discover_files(tmp.path(), "snap-1").unwrap();

        let root_id = &nodes[0].id;
        let src_node = nodes.iter().find(|n| n.name == "src").unwrap();
        assert_eq!(src_node.parent_id.as_ref().unwrap(), root_id);
        assert_eq!(src_node.depth, 1);
    }

    #[test]
    fn ignores_git_directory() {
        let tmp = tempfile::tempdir().unwrap();
        create_mock_project(tmp.path());
        let nodes = discover_files(tmp.path(), "snap-1").unwrap();
        let git_nodes: Vec<&Node> = nodes.iter().filter(|n| n.name == ".git").collect();
        assert!(git_nodes.is_empty());
    }

    #[test]
    fn depth_increases_with_nesting() {
        let tmp = tempfile::tempdir().unwrap();
        create_mock_project(tmp.path());
        let nodes = discover_files(tmp.path(), "snap-1").unwrap();

        let mod_rs = nodes.iter().find(|n| n.name == "mod.rs").unwrap();
        assert_eq!(mod_rs.depth, 3); // root(0) -> src(1) -> utils(2) -> mod.rs(3)
    }
}
