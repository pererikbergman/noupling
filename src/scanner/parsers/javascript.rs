use tree_sitter::Parser;

use super::{ImportEntry, LanguageParser};
use super::typescript::resolve_typescript_import;

pub struct JavaScriptParser;

impl LanguageParser for JavaScriptParser {
    fn parse(&self, source: &str) -> Vec<ImportEntry> {
        let mut parser = Parser::new();
        let js_lang: tree_sitter::Language = tree_sitter_javascript::LANGUAGE.into();
        parser
            .set_language(&js_lang)
            .expect("Failed to set JavaScript language");

        let tree = match parser.parse(source, None) {
            Some(t) => t,
            None => return Vec::new(),
        };

        let mut imports = Vec::new();
        // JavaScript uses the same import_statement AST node as TypeScript
        collect_js_imports(tree.root_node(), source, &mut imports);
        imports
    }

    fn resolve(
        &self,
        import_path: &str,
        source_file: &str,
        known_paths: &[String],
    ) -> Option<String> {
        resolve_typescript_import(import_path, source_file, known_paths)
    }
}

fn collect_js_imports(node: tree_sitter::Node, source: &str, imports: &mut Vec<ImportEntry>) {
    if node.kind() == "import_statement" {
        let line_number = (node.start_position().row + 1) as i32;
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "string" {
                let text = node_text(child, source);
                let path = text.trim_matches(|c| c == '"' || c == '\'').to_string();
                if !path.is_empty() {
                    imports.push(ImportEntry { path, line_number });
                }
                return;
            }
        }
        return;
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_js_imports(child, source, imports);
    }
}

fn node_text(node: tree_sitter::Node, source: &str) -> String {
    source[node.byte_range()].to_string()
}

// ── Tests ──────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn js_parses_es_import() {
        let source = "import { helper } from './utils';";
        let imports = JavaScriptParser.parse(source);
        assert_eq!(imports.len(), 1);
        assert_eq!(imports[0].path, "./utils");
    }

    #[test]
    fn js_parses_default_import() {
        let source = "import express from 'express';";
        let imports = JavaScriptParser.parse(source);
        assert_eq!(imports.len(), 1);
        assert_eq!(imports[0].path, "express");
    }

    #[test]
    fn js_handles_empty_source() {
        let imports = JavaScriptParser.parse("");
        assert!(imports.is_empty());
    }
}
