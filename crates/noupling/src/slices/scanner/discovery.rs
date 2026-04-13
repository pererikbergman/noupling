use anyhow::Result;
use std::path::Path;
use uuid::Uuid;

use crate::core::{Module, ModuleType};
use crate::settings::Settings;

pub fn discover_files(root: &Path, snapshot_id: &str) -> Result<Vec<Module>> {
    let settings = Settings::load(root).unwrap_or_default();
    discover_files_with_settings(root, snapshot_id, &settings)
}

pub fn discover_files_with_settings(
    root: &Path,
    snapshot_id: &str,
    settings: &Settings,
) -> Result<Vec<Module>> {
    let mut nodes = Vec::new();
    let root_canonical = root.canonicalize()?;
    walk_directory(&root_canonical, snapshot_id, &root_canonical, &mut nodes, settings)?;
    Ok(nodes)
}

fn walk_directory(
    dir: &Path,
    snapshot_id: &str,
    root: &Path,
    modules: &mut Vec<Module>,
    settings: &Settings,
) -> Result<()> {
    let mut entries: Vec<_> = std::fs::read_dir(dir)?
        .filter_map(|e| e.ok())
        .collect();
    entries.sort_by_key(|e| e.file_name());

    for entry in entries {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();

        if path.is_dir() {
            if settings.ignored_dirs.iter().any(|d| d == &name) {
                continue;
            }
            walk_directory(&path, snapshot_id, root, modules, settings)?;
        } else {
            let is_source = path
                .extension()
                .and_then(|e| e.to_str())
                .map(|ext| settings.source_extensions.iter().any(|s| s == ext))
                .unwrap_or(false);

            if !is_source {
                continue;
            }

            let rel_path = path
                .strip_prefix(root)
                .unwrap_or(&path);

            let depth = rel_path.components().count() as i32;

            modules.push(Module {
                id: Uuid::new_v4().to_string(),
                snapshot_id: snapshot_id.to_string(),
                parent_id: None,
                name,
                path: rel_path.to_string_lossy().to_string(),
                module_type: ModuleType::File,
                depth,
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

        // Non-source files should be excluded
        fs::write(src.join("README.md"), "# Readme").unwrap();

        // Should be ignored
        let git = dir.join(".git");
        fs::create_dir_all(&git).unwrap();
        fs::write(git.join("HEAD"), "ref: refs/heads/main").unwrap();
    }

    #[test]
    fn discovers_only_rs_files() {
        let tmp = tempfile::tempdir().unwrap();
        create_mock_project(tmp.path());

        let nodes = discover_files(tmp.path(), "snap-1").unwrap();

        assert_eq!(nodes.len(), 3, "Expected 3 .rs files, got: {:?}", nodes.iter().map(|n| &n.name).collect::<Vec<_>>());
        assert!(nodes.iter().all(|n| n.name.ends_with(".rs")));
        assert!(nodes.iter().all(|n| matches!(n.module_type, ModuleType::File)));
    }

    #[test]
    fn no_directory_nodes() {
        let tmp = tempfile::tempdir().unwrap();
        create_mock_project(tmp.path());

        let nodes = discover_files(tmp.path(), "snap-1").unwrap();
        let dirs: Vec<&Module> = nodes.iter().filter(|n| matches!(n.module_type, ModuleType::Dir)).collect();
        assert!(dirs.is_empty());
    }

    #[test]
    fn depth_is_relative_to_root() {
        let tmp = tempfile::tempdir().unwrap();
        create_mock_project(tmp.path());

        let nodes = discover_files(tmp.path(), "snap-1").unwrap();

        let main_rs = nodes.iter().find(|n| n.name == "main.rs").unwrap();
        assert_eq!(main_rs.depth, 2); // src/main.rs = depth 2

        let mod_rs = nodes.iter().find(|n| n.name == "mod.rs").unwrap();
        assert_eq!(mod_rs.depth, 3); // src/utils/mod.rs = depth 3
    }

    #[test]
    fn ignores_git_directory() {
        let tmp = tempfile::tempdir().unwrap();
        create_mock_project(tmp.path());
        let nodes = discover_files(tmp.path(), "snap-1").unwrap();
        assert!(nodes.iter().all(|n| !n.path.contains(".git")));
    }
}
