use std::path::Path;
use tree_sitter::Parser;

use super::{ImportEntry, LanguageParser};

pub struct DartParser;

impl LanguageParser for DartParser {
    fn parse(&self, source: &str) -> Vec<ImportEntry> {
        let mut parser = Parser::new();
        parser
            .set_language(&tree_sitter_dart::LANGUAGE.into())
            .expect("Failed to set Dart language");

        let tree = match parser.parse(source, None) {
            Some(t) => t,
            None => return Vec::new(),
        };

        let mut imports = Vec::new();
        collect_dart_imports(tree.root_node(), source, &mut imports);
        imports
    }

    fn resolve(
        &self,
        import_path: &str,
        source_file: &str,
        known_paths: &[String],
    ) -> Option<String> {
        resolve_dart_import(import_path, source_file, known_paths)
    }
}

fn collect_dart_imports(node: tree_sitter::Node, source: &str, imports: &mut Vec<ImportEntry>) {
    if node.kind() == "import_or_export" {
        let line_number = (node.start_position().row + 1) as i32;
        if let Some(text) = find_first_string_literal(node, source) {
            let path = text.trim_matches('\'').trim_matches('"').to_string();
            // Skip dart: stdlib imports — they are never internal project files
            if !path.starts_with("dart:") && !path.is_empty() {
                imports.push(ImportEntry { path, line_number });
            }
        }
        return;
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_dart_imports(child, source, imports);
    }
}

fn find_first_string_literal(node: tree_sitter::Node, source: &str) -> Option<String> {
    if node.kind() == "string_literal" {
        return Some(node_text(node, source));
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if let Some(text) = find_first_string_literal(child, source) {
            return Some(text);
        }
    }
    None
}

fn node_text(node: tree_sitter::Node, source: &str) -> String {
    source[node.byte_range()].to_string()
}

// ── Resolver ──────────────────────────────────────────────────────────────────

fn normalize_path(path: &str) -> String {
    path.replace('\\', "/")
}

fn resolve_dart_import(
    import_path: &str,
    source_file: &str,
    known_paths: &[String],
) -> Option<String> {
    if import_path.starts_with("package:") {
        let after_package = import_path.strip_prefix("package:")?;
        let after_name = after_package.split_once('/')?.1;
        let candidate = format!("lib/{}", after_name);
        if let Some(found) = known_paths.iter().find(|p| p.ends_with(&candidate)) {
            return Some(found.clone());
        }
        return known_paths
            .iter()
            .find(|p| p.ends_with(after_name))
            .cloned();
    }

    let source_dir = Path::new(source_file).parent()?;
    let resolved = normalize_path(&source_dir.join(import_path).to_string_lossy());
    known_paths
        .iter()
        .find(|p| **p == resolved || p.ends_with(&resolved))
        .cloned()
}

// ── Tests ──────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dart_parses_imports() {
        let source = "import 'package:flutter/material.dart';\nimport 'src/utils.dart';\n";
        let imports = DartParser.parse(source);
        assert_eq!(imports.len(), 2);
    }

    #[test]
    fn dart_skips_stdlib() {
        let source = "import 'dart:core';\nimport 'dart:async';\nimport 'src/model.dart';\n";
        let imports = DartParser.parse(source);
        assert_eq!(imports.len(), 1);
    }

    #[test]
    fn dart_handles_empty_source() {
        let imports = DartParser.parse("");
        assert!(imports.is_empty());
    }
}
