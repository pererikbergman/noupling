//! Source file discovery, Tree-sitter parsing, and import resolution.
//!
//! Supports 14 languages via the `parsers` registry: C#, Dart, Go, Haskell,
//! Java, JavaScript, Kotlin, PHP, Python, Ruby, Rust, Swift, TypeScript, and Zig.

mod discovery;
pub mod parsers;

pub use discovery::discover_files;

use crate::core::{Dependency, Module};
use anyhow::Result;
use rayon::prelude::*;
use std::collections::HashMap;
use std::path::Path;

/// The result of scanning a project directory.
pub struct ScanResult {
    /// All discovered source modules.
    pub modules: Vec<Module>,
    /// All resolved import dependencies between modules.
    pub dependencies: Vec<Dependency>,
    /// Number of imports suppressed by `noupling:ignore` comments.
    pub suppressed_count: usize,
    /// Per-module count of external (unresolved) imports.
    pub external_imports: Vec<ExternalImportCount>,
}

/// Count of imports in a module that don't resolve to any project file.
pub struct ExternalImportCount {
    /// Path of the module containing the unresolved imports.
    pub module_path: String,
    /// Number of unique external imports.
    pub count: usize,
}

/// Per-module count of abstract vs concrete type declarations.
///
/// Emitted by the scanner alongside dependencies; consumed by
/// `analyzer::compute_abstractness` to derive per-directory abstractness.
pub struct ModuleTypeCounts {
    pub module_path: String,
    pub counts: parsers::TypeCounts,
}

/// Check if an import line is suppressed by a `noupling:ignore` comment.
/// Checks the import line itself for an inline comment, and the line above
/// if it is a standalone comment line (starts with //, #, or --).
fn is_suppressed(source: &str, line_number: i32) -> bool {
    let lines: Vec<&str> = source.lines().collect();
    let idx = (line_number - 1) as usize;

    // Check the import line itself for an inline comment
    if idx < lines.len() && lines[idx].contains("noupling:ignore") {
        return true;
    }

    // Check the line above only if it's a standalone comment
    if idx > 0 && (idx - 1) < lines.len() {
        let above = lines[idx - 1].trim();
        let is_comment =
            above.starts_with("//") || above.starts_with('#') || above.starts_with("--");
        if is_comment && above.contains("noupling:ignore") {
            return true;
        }
    }

    false
}

/// Recompute per-module type counts for the current source files.
///
/// Used by commands that load modules from SQLite (which doesn't persist
/// type counts) but need to feed abstractness analysis. Re-reads each
/// source file and invokes the language adapter's `count_type_declarations`.
/// Modules whose source files can't be read, or whose extension has no
/// adapter, are silently skipped.
pub fn recompute_type_counts(root: &Path, modules: &[Module]) -> Vec<ModuleTypeCounts> {
    let registry = parsers::registry();
    let ext_map: HashMap<&str, &dyn parsers::LanguageParser> = registry
        .iter()
        .map(|(ext, adapter)| (*ext, adapter.as_ref()))
        .collect();
    modules
        .par_iter()
        .filter_map(|m| {
            let rel = Path::new(&m.path);
            let ext = rel.extension().and_then(|e| e.to_str()).unwrap_or("");
            let adapter = ext_map.get(ext)?;
            let source = std::fs::read_to_string(root.join(rel)).ok()?;
            let counts = adapter.count_type_declarations(&source);
            if counts.abstract_count + counts.concrete_count == 0 {
                return None;
            }
            Some(ModuleTypeCounts {
                module_path: m.path.clone(),
                counts,
            })
        })
        .collect()
}

pub fn scan_project(
    root: &Path,
    snapshot_id: &str,
    allow_inline_suppression: bool,
) -> Result<ScanResult> {
    let root = root.canonicalize()?;
    let modules = discover_files(&root, snapshot_id)?;

    let all_paths: Vec<String> = modules.iter().map(|m| m.path.clone()).collect();

    // Build a map from extension -> adapter (avoids cloning boxes repeatedly)
    let registry = parsers::registry();
    let ext_map: HashMap<&str, &dyn parsers::LanguageParser> = registry
        .iter()
        .map(|(ext, adapter)| (*ext, adapter.as_ref()))
        .collect();

    let per_file: Vec<(Vec<Dependency>, usize, ExternalImportCount)> = modules
        .par_iter()
        .filter_map(|module| {
            let rel_path = Path::new(&module.path);
            let ext = rel_path.extension().and_then(|e| e.to_str()).unwrap_or("");
            let adapter = ext_map.get(ext)?;
            let abs_path = root.join(rel_path);
            let source = std::fs::read_to_string(&abs_path).ok()?;
            let imports = adapter.parse(&source);

            let mut suppressed = 0usize;
            let mut external_count = 0usize;
            let deps: Vec<Dependency> = imports
                .iter()
                .filter_map(|entry| {
                    if allow_inline_suppression && is_suppressed(&source, entry.line_number) {
                        suppressed += 1;
                        return None;
                    }
                    let resolved = adapter.resolve(&entry.path, &module.path, &all_paths);
                    match resolved {
                        Some(path) => {
                            let to_module = modules.iter().find(|m| m.path == path)?;
                            // Defence in depth: a file importing itself is never a
                            // meaningful coupling event. Drop self-edges so a future
                            // resolver bug can't surface them as cross-module deps.
                            if to_module.id == module.id {
                                return None;
                            }
                            Some(Dependency {
                                from_module_id: module.id.clone(),
                                to_module_id: to_module.id.clone(),
                                line_number: entry.line_number,
                            })
                        }
                        None => {
                            external_count += 1;
                            None
                        }
                    }
                })
                .collect();
            let ext_count = ExternalImportCount {
                module_path: module.path.clone(),
                count: external_count,
            };
            Some((deps, suppressed, ext_count))
        })
        .collect();

    let mut dependencies = Vec::new();
    let mut suppressed_count = 0usize;
    let mut external_imports = Vec::new();
    for (deps, suppressed, ext) in per_file {
        dependencies.extend(deps);
        suppressed_count += suppressed;
        if ext.count > 0 {
            external_imports.push(ext);
        }
    }

    Ok(ScanResult {
        modules,
        dependencies,
        suppressed_count,
        external_imports,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::ModuleType;

    #[test]
    fn scan_project_discovers_modules_and_deps() {
        let fixture =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/mock_rust_project");
        let result = scan_project(&fixture, "test-snap", true).unwrap();

        // Only source files, no directories
        assert!(result
            .modules
            .iter()
            .all(|m| matches!(m.module_type, ModuleType::File)));
        // main.rs, mod.rs, helper.rs = 3 files
        assert_eq!(
            result.modules.len(),
            3,
            "files: {:?}",
            result.modules.iter().map(|m| &m.name).collect::<Vec<_>>()
        );

        // main.rs has `use crate::modules::helper` which should resolve to helper.rs
        assert!(
            !result.dependencies.is_empty(),
            "Expected at least one dependency from main.rs -> helper.rs"
        );
    }

    #[test]
    fn scan_project_parallel_produces_consistent_results() {
        let fixture =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/mock_rust_project");

        let r1 = scan_project(&fixture, "snap-1", true).unwrap();
        let r2 = scan_project(&fixture, "snap-2", true).unwrap();

        assert_eq!(r1.modules.len(), r2.modules.len());
        assert_eq!(r1.dependencies.len(), r2.dependencies.len());
    }

    #[test]
    fn is_suppressed_same_line() {
        let source = "use crate::foo; // noupling:ignore\nuse crate::bar;\n";
        assert!(is_suppressed(source, 1));
        assert!(!is_suppressed(source, 2));
    }

    #[test]
    fn is_suppressed_line_above() {
        let source = "// noupling:ignore\nuse crate::foo;\nuse crate::bar;\n";
        assert!(is_suppressed(source, 2));
        assert!(!is_suppressed(source, 3));
    }

    #[test]
    fn is_suppressed_python_comment() {
        let source = "# noupling:ignore\nimport foo\nimport bar\n";
        assert!(is_suppressed(source, 2));
        assert!(!is_suppressed(source, 3));
    }

    #[test]
    fn is_suppressed_haskell_comment() {
        let source = "-- noupling:ignore\nimport Data.List\nimport Data.Map\n";
        assert!(is_suppressed(source, 2));
        assert!(!is_suppressed(source, 3));
    }

    #[test]
    fn is_suppressed_inline_same_line_kotlin() {
        let source = "import com.example.foo // noupling:ignore\nimport com.example.bar\n";
        assert!(is_suppressed(source, 1));
        assert!(!is_suppressed(source, 2));
    }

    #[test]
    fn is_suppressed_not_triggered_without_comment() {
        let source = "use crate::foo;\nuse crate::bar;\n";
        assert!(!is_suppressed(source, 1));
        assert!(!is_suppressed(source, 2));
    }

    #[test]
    fn recompute_type_counts_finds_rust_trait_and_struct() {
        use std::fs;
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("src");
        fs::create_dir_all(&src).unwrap();
        fs::write(src.join("lib.rs"), "pub trait T {}\npub struct S {}\n").unwrap();

        let result = scan_project(dir.path(), "test-types", true).unwrap();
        let counts = recompute_type_counts(dir.path(), &result.modules);

        let lib_counts = counts
            .iter()
            .find(|t| t.module_path.ends_with("src/lib.rs"))
            .expect("expected type counts for src/lib.rs");
        assert_eq!(lib_counts.counts.abstract_count, 1);
        assert_eq!(lib_counts.counts.concrete_count, 1);
    }

    #[test]
    fn stdlib_python_import_does_not_create_self_edge() {
        use std::fs;
        let dir = tempfile::tempdir().unwrap();
        let stages = dir.path().join("src").join("stages");
        fs::create_dir_all(&stages).unwrap();
        // `structure.py` ends with the literal string "re.py" — before the resolver
        // fix, `import re` would substring-match this file and produce a self-edge.
        fs::write(stages.join("structure.py"), "import re\nimport os\n").unwrap();
        fs::write(stages.join("chords.py"), "import os\n").unwrap();

        let result = scan_project(dir.path(), "test-self-edge", true).unwrap();

        let self_edges: Vec<_> = result
            .dependencies
            .iter()
            .filter(|d| d.from_module_id == d.to_module_id)
            .collect();
        assert!(
            self_edges.is_empty(),
            "stdlib imports must not produce self-edges, got {:?}",
            self_edges
        );
        assert!(
            result.dependencies.is_empty(),
            "stdlib-only imports must not produce any deps, got {:?}",
            result.dependencies
        );
    }
}
