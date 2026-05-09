use std::path::Path;
use tree_sitter::Parser;

use super::{ImportEntry, LanguageParser};

pub struct RustParser;

impl LanguageParser for RustParser {
    fn parse(&self, source: &str) -> Vec<ImportEntry> {
        let mut parser = Parser::new();
        parser
            .set_language(&tree_sitter_rust::LANGUAGE.into())
            .expect("Failed to set Rust language");

        let tree = match parser.parse(source, None) {
            Some(t) => t,
            None => return Vec::new(),
        };

        let mut imports = Vec::new();
        collect_use_declarations(tree.root_node(), source, &mut imports);
        imports
    }

    fn resolve(
        &self,
        import_path: &str,
        source_file: &str,
        known_paths: &[String],
    ) -> Option<String> {
        resolve_rust_import(import_path, source_file, known_paths)
    }
}

fn collect_use_declarations(node: tree_sitter::Node, source: &str, imports: &mut Vec<ImportEntry>) {
    if node.kind() == "use_declaration" {
        let line_number = (node.start_position().row + 1) as i32;
        extract_paths_from_use(node, source, line_number, imports);
        return;
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_use_declarations(child, source, imports);
    }
}

fn extract_paths_from_use(
    node: tree_sitter::Node,
    source: &str,
    line_number: i32,
    imports: &mut Vec<ImportEntry>,
) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "scoped_identifier" | "identifier" | "scoped_use_list" | "use_wildcard" => {
                collect_paths_from_node(child, source, line_number, imports);
            }
            _ => {}
        }
    }
}

fn collect_paths_from_node(
    node: tree_sitter::Node,
    source: &str,
    line_number: i32,
    imports: &mut Vec<ImportEntry>,
) {
    match node.kind() {
        "scoped_use_list" => {
            let mut prefix = String::new();
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                match child.kind() {
                    "scoped_identifier" | "identifier" => {
                        prefix = node_text(child, source);
                    }
                    "use_list" => {
                        let mut list_cursor = child.walk();
                        for list_child in child.children(&mut list_cursor) {
                            match list_child.kind() {
                                "identifier" => {
                                    let name = node_text(list_child, source);
                                    let full_path = if prefix.is_empty() {
                                        name
                                    } else {
                                        format!("{}::{}", prefix, name)
                                    };
                                    imports.push(ImportEntry {
                                        path: full_path,
                                        line_number,
                                    });
                                }
                                "scoped_identifier" => {
                                    let name = node_text(list_child, source);
                                    let full_path = if prefix.is_empty() {
                                        name
                                    } else {
                                        format!("{}::{}", prefix, name)
                                    };
                                    imports.push(ImportEntry {
                                        path: full_path,
                                        line_number,
                                    });
                                }
                                "self" => {
                                    imports.push(ImportEntry {
                                        path: prefix.clone(),
                                        line_number,
                                    });
                                }
                                _ => {}
                            }
                        }
                        return;
                    }
                    "::" => {}
                    _ => {}
                }
            }
            let text = node_text(node, source);
            imports.push(ImportEntry {
                path: text,
                line_number,
            });
        }
        "scoped_identifier" | "identifier" => {
            let text = node_text(node, source);
            imports.push(ImportEntry {
                path: text,
                line_number,
            });
        }
        "use_wildcard" => {
            let text = node_text(node, source);
            imports.push(ImportEntry {
                path: text,
                line_number,
            });
        }
        _ => {}
    }
}

fn node_text(node: tree_sitter::Node, source: &str) -> String {
    source[node.byte_range()].to_string()
}

// ── Resolver ──────────────────────────────────────────────────────────────────

fn normalize_path(path: &str) -> String {
    path.replace('\\', "/")
}

fn resolve_rust_import(
    import_path: &str,
    source_file: &str,
    known_paths: &[String],
) -> Option<String> {
    let segments: Vec<&str> = if import_path.starts_with("crate::") {
        import_path.strip_prefix("crate::")?.split("::").collect()
    } else if import_path.starts_with("super::") {
        return resolve_super_import(import_path, source_file, known_paths);
    } else if import_path.starts_with("self::") {
        return resolve_self_import(import_path, source_file, known_paths);
    } else {
        return None;
    };

    let src_root = find_src_root(source_file)?;
    try_resolve_segments(&segments, &src_root, known_paths)
}

fn find_src_root(source_file: &str) -> Option<String> {
    let source_path = Path::new(source_file);
    let mut current = source_path.parent()?;
    loop {
        if current.file_name()?.to_str()? == "src" {
            return Some(normalize_path(&current.to_string_lossy()));
        }
        current = current.parent()?;
    }
}

fn try_resolve_segments(
    segments: &[&str],
    src_root: &str,
    known_paths: &[String],
) -> Option<String> {
    if segments.is_empty() {
        return None;
    }

    let base = Path::new(src_root);

    // Try as a file: segments joined as directories, last segment as .rs file
    let mut file_path = base.to_path_buf();
    for (i, seg) in segments.iter().enumerate() {
        if i == segments.len() - 1 {
            file_path.push(format!("{}.rs", seg));
        } else {
            file_path.push(seg);
        }
    }
    let candidate = normalize_path(&file_path.to_string_lossy());
    if known_paths.contains(&candidate) {
        return Some(candidate);
    }

    // Try as a module directory with mod.rs
    let mut mod_path = base.to_path_buf();
    for seg in segments {
        mod_path.push(seg);
    }
    mod_path.push("mod.rs");
    let candidate = normalize_path(&mod_path.to_string_lossy());
    if known_paths.contains(&candidate) {
        return Some(candidate);
    }

    // Try without the last segment (it might be a type/function, not a file)
    if segments.len() > 1 {
        let parent_segments = &segments[..segments.len() - 1];
        return try_resolve_segments(parent_segments, src_root, known_paths);
    }

    None
}

fn resolve_super_import(
    import_path: &str,
    source_file: &str,
    known_paths: &[String],
) -> Option<String> {
    let source_dir = Path::new(source_file).parent()?;
    let remaining = import_path.strip_prefix("super::")?;

    let is_mod_rs = Path::new(source_file)
        .file_name()
        .map(|f| f == "mod.rs" || f == "lib.rs")
        .unwrap_or(false);

    let base = if is_mod_rs {
        source_dir.parent()?
    } else {
        source_dir
    };

    let segments: Vec<&str> = remaining.split("::").collect();
    let src_root = normalize_path(&base.to_string_lossy());

    try_resolve_segments(&segments, &src_root, known_paths)
}

fn resolve_self_import(
    import_path: &str,
    source_file: &str,
    known_paths: &[String],
) -> Option<String> {
    let source_dir = Path::new(source_file).parent()?;
    let remaining = import_path.strip_prefix("self::")?;

    let segments: Vec<&str> = remaining.split("::").collect();
    let src_root = normalize_path(&source_dir.to_string_lossy());

    try_resolve_segments(&segments, &src_root, known_paths)
}

// ── Tests ──────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn parse(source: &str) -> Vec<ImportEntry> {
        RustParser.parse(source)
    }

    fn resolve(import_path: &str, source_file: &str, known_paths: &[String]) -> Option<String> {
        RustParser.resolve(import_path, source_file, known_paths)
    }

    fn project_paths() -> Vec<String> {
        vec![
            "src/main.rs".to_string(),
            "src/core/mod.rs".to_string(),
            "src/core/error.rs".to_string(),
            "src/utils.rs".to_string(),
            "src/slices/scanner/mod.rs".to_string(),
            "src/slices/scanner/parser.rs".to_string(),
        ]
    }

    // Parser tests

    #[test]
    fn parses_simple_use() {
        let source = "use std::collections::HashMap;";
        let imports = parse(source);
        assert_eq!(imports.len(), 1);
        assert_eq!(imports[0].path, "std::collections::HashMap");
        assert_eq!(imports[0].line_number, 1);
    }

    #[test]
    fn parses_use_with_line_number() {
        let source = "\nuse std::io;\n";
        let imports = parse(source);
        assert_eq!(imports.len(), 1);
        assert_eq!(imports[0].path, "std::io");
        assert_eq!(imports[0].line_number, 2);
    }

    #[test]
    fn parses_crate_use() {
        let source = "use crate::core::Node;";
        let imports = parse(source);
        assert_eq!(imports.len(), 1);
        assert_eq!(imports[0].path, "crate::core::Node");
    }

    #[test]
    fn parses_use_list() {
        let source = "use std::collections::{HashMap, HashSet};";
        let imports = parse(source);
        assert_eq!(imports.len(), 2);
        let paths: Vec<&str> = imports.iter().map(|i| i.path.as_str()).collect();
        assert!(paths.contains(&"std::collections::HashMap"));
        assert!(paths.contains(&"std::collections::HashSet"));
    }

    #[test]
    fn parses_glob_use() {
        let source = "use std::io::*;";
        let imports = parse(source);
        assert_eq!(imports.len(), 1);
        assert_eq!(imports[0].path, "std::io::*");
    }

    #[test]
    fn parses_multiple_use_statements() {
        let source = "use std::io;\nuse std::fs;\nuse crate::utils;\n";
        let imports = parse(source);
        assert_eq!(imports.len(), 3);
        assert_eq!(imports[0].line_number, 1);
        assert_eq!(imports[1].line_number, 2);
        assert_eq!(imports[2].line_number, 3);
    }

    #[test]
    fn handles_empty_source() {
        let imports = parse("");
        assert!(imports.is_empty());
    }

    #[test]
    fn ignores_non_use_code() {
        let source = "fn main() { let x = 42; }";
        let imports = parse(source);
        assert!(imports.is_empty());
    }

    // Resolver tests

    #[test]
    fn resolves_crate_module_path() {
        let paths = project_paths();
        let root = Path::new("");
        let _ = root; // unused but kept for parity with original tests
        let result = resolve("crate::core", "src/main.rs", &paths);
        assert_eq!(result, Some("src/core/mod.rs".to_string()));
    }

    #[test]
    fn resolves_crate_file_path() {
        let paths = project_paths();
        let result = resolve("crate::utils", "src/main.rs", &paths);
        assert_eq!(result, Some("src/utils.rs".to_string()));
    }

    #[test]
    fn resolves_crate_nested_path() {
        let paths = project_paths();
        let result = resolve("crate::core::error", "src/main.rs", &paths);
        assert_eq!(result, Some("src/core/error.rs".to_string()));
    }

    #[test]
    fn resolves_crate_type_to_parent_file() {
        let paths = project_paths();
        let result = resolve("crate::core::error::CoreError", "src/main.rs", &paths);
        assert_eq!(result, Some("src/core/error.rs".to_string()));
    }

    #[test]
    fn returns_none_for_external_crate() {
        let paths = project_paths();
        let result = resolve("std::collections::HashMap", "src/main.rs", &paths);
        assert!(result.is_none());
    }

    #[test]
    fn returns_none_for_serde() {
        let paths = project_paths();
        let result = resolve("serde::Deserialize", "src/main.rs", &paths);
        assert!(result.is_none());
    }

    #[test]
    fn resolves_super_import_from_file() {
        let paths = project_paths();
        let result = resolve("super::parser", "src/slices/scanner/mod.rs", &paths);
        // super from mod.rs goes up to slices/, no parser there
        assert!(result.is_none());
    }

    #[test]
    fn resolves_self_import() {
        let paths = project_paths();
        let result = resolve("self::parser", "src/slices/scanner/mod.rs", &paths);
        assert_eq!(result, Some("src/slices/scanner/parser.rs".to_string()));
    }
}
