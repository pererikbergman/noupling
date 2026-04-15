//! Source file discovery, Tree-sitter parsing, and import resolution.
//!
//! Supports 11 languages: C#, Go, Haskell, Java, JavaScript, Kotlin,
//! Python, Rust, Swift, TypeScript, and Zig.

mod discovery;
mod parser;
mod resolver;

pub use discovery::discover_files;
pub use parser::parse_rust_imports;
pub use resolver::resolve_import;

use crate::core::{Dependency, Module};
use anyhow::Result;
use rayon::prelude::*;
use std::path::Path;

/// The result of scanning a project directory.
pub struct ScanResult {
    /// All discovered source modules.
    pub modules: Vec<Module>,
    /// All resolved import dependencies between modules.
    pub dependencies: Vec<Dependency>,
    /// Number of imports suppressed by `noupling:ignore` comments.
    pub suppressed_count: usize,
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

pub fn scan_project(
    root: &Path,
    snapshot_id: &str,
    allow_inline_suppression: bool,
) -> Result<ScanResult> {
    let root = root.canonicalize()?;
    let modules = discover_files(&root, snapshot_id)?;

    let all_paths: Vec<String> = modules.iter().map(|m| m.path.clone()).collect();

    let per_file: Vec<(Vec<Dependency>, usize)> = modules
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
                "go" => parser::parse_go_imports(&source),
                "hs" => parser::parse_haskell_imports(&source),
                "java" => parser::parse_java_imports(&source),
                "js" | "jsx" => parser::parse_javascript_imports(&source),
                "py" => parser::parse_python_imports(&source),
                "zig" => parser::parse_zig_imports(&source),
                _ => return None,
            };
            let mut suppressed = 0usize;
            let deps: Vec<Dependency> = imports
                .iter()
                .filter_map(|entry| {
                    if allow_inline_suppression && is_suppressed(&source, entry.line_number) {
                        suppressed += 1;
                        return None;
                    }
                    let resolved =
                        resolve_import(&entry.path, &module.path, Path::new(""), &all_paths)?;
                    let to_module = modules.iter().find(|m| m.path == resolved)?;
                    Some(Dependency {
                        from_module_id: module.id.clone(),
                        to_module_id: to_module.id.clone(),
                        line_number: entry.line_number,
                    })
                })
                .collect();
            Some((deps, suppressed))
        })
        .collect();

    let mut dependencies = Vec::new();
    let mut suppressed_count = 0usize;
    for (deps, suppressed) in per_file {
        dependencies.extend(deps);
        suppressed_count += suppressed;
    }

    Ok(ScanResult {
        modules,
        dependencies,
        suppressed_count,
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
}
