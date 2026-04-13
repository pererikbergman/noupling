mod discovery;
mod parser;
mod resolver;

pub use discovery::discover_files;
pub use parser::parse_rust_imports;
pub use parser::ImportEntry;
pub use resolver::resolve_import;

use crate::core::{Dependency, Module};
use anyhow::Result;
use rayon::prelude::*;
use std::path::Path;

pub struct ScanResult {
    pub modules: Vec<Module>,
    pub dependencies: Vec<Dependency>,
}

pub fn scan_project(root: &Path, snapshot_id: &str) -> Result<ScanResult> {
    let root = root.canonicalize()?;
    let modules = discover_files(&root, snapshot_id)?;

    let all_paths: Vec<String> = modules.iter().map(|m| m.path.clone()).collect();

    let dependencies: Vec<Dependency> = modules
        .par_iter()
        .filter_map(|module| {
            let rel_path = Path::new(&module.path);
            let ext = rel_path.extension().and_then(|e| e.to_str()).unwrap_or("");
            let abs_path = root.join(rel_path);
            let source = std::fs::read_to_string(&abs_path).ok()?;
            let imports = match ext {
                "rs" => parse_rust_imports(&source),
                "kt" | "kts" => parser::parse_kotlin_imports(&source),
                "ts" => parser::parse_typescript_imports(&source),
                "tsx" => parser::parse_tsx_imports(&source),
                "swift" => parser::parse_swift_imports(&source),
                "cs" => parser::parse_csharp_imports(&source),
                _ => return None,
            };
            let deps: Vec<Dependency> = imports
                .iter()
                .filter_map(|entry| {
                    let resolved = resolve_import(&entry.path, &module.path, Path::new(""), &all_paths)?;
                    let to_module = modules.iter().find(|m| m.path == resolved)?;
                    Some(Dependency {
                        from_module_id: module.id.clone(),
                        to_module_id: to_module.id.clone(),
                        line_number: entry.line_number,
                    })
                })
                .collect();
            Some(deps)
        })
        .flatten()
        .collect();

    Ok(ScanResult {
        modules,
        dependencies,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::ModuleType;

    #[test]
    fn scan_project_discovers_modules_and_deps() {
        let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/mock_rust_project");
        let result = scan_project(&fixture, "test-snap").unwrap();

        // Only source files, no directories
        assert!(result.modules.iter().all(|m| matches!(m.module_type, ModuleType::File)));
        // main.rs, mod.rs, helper.rs = 3 files
        assert_eq!(result.modules.len(), 3, "files: {:?}", result.modules.iter().map(|m| &m.name).collect::<Vec<_>>());

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

        let r1 = scan_project(&fixture, "snap-1").unwrap();
        let r2 = scan_project(&fixture, "snap-2").unwrap();

        assert_eq!(r1.modules.len(), r2.modules.len());
        assert_eq!(r1.dependencies.len(), r2.dependencies.len());
    }
}
