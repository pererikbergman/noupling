use std::path::Path;
use tree_sitter::Parser;

use super::{ImportEntry, LanguageParser, TypeCounts};

/// TypeScript adapter (`.ts` files).
pub struct TypeScriptParser;

/// TSX adapter (`.tsx` files) — same AST logic, different grammar.
pub struct TsxParser;

impl LanguageParser for TypeScriptParser {
    fn parse(&self, source: &str) -> Vec<ImportEntry> {
        parse_with_ts_grammar(source, false)
    }

    fn resolve(
        &self,
        import_path: &str,
        source_file: &str,
        known_paths: &[String],
    ) -> Option<String> {
        resolve_typescript_import(import_path, source_file, known_paths)
    }

    fn count_type_declarations(&self, source: &str) -> TypeCounts {
        count_typescript_types(source, false)
    }
}

impl LanguageParser for TsxParser {
    fn parse(&self, source: &str) -> Vec<ImportEntry> {
        parse_with_ts_grammar(source, true)
    }

    fn resolve(
        &self,
        import_path: &str,
        source_file: &str,
        known_paths: &[String],
    ) -> Option<String> {
        resolve_typescript_import(import_path, source_file, known_paths)
    }

    fn count_type_declarations(&self, source: &str) -> TypeCounts {
        count_typescript_types(source, true)
    }
}

fn count_typescript_types(source: &str, tsx: bool) -> TypeCounts {
    let mut parser = Parser::new();
    let lang: tree_sitter::Language = if tsx {
        tree_sitter_typescript::LANGUAGE_TSX.into()
    } else {
        tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into()
    };
    parser
        .set_language(&lang)
        .expect("Failed to set TypeScript language");
    let tree = match parser.parse(source, None) {
        Some(t) => t,
        None => return TypeCounts::default(),
    };
    let mut counts = TypeCounts::default();
    count_ts_types(tree.root_node(), source, &mut counts);
    counts
}

/// Declared at the top of the module, directly or behind `export`.
fn is_module_level(node: tree_sitter::Node) -> bool {
    match node.parent() {
        None => true,
        Some(p) if p.kind() == "program" => true,
        Some(p) if p.kind() == "export_statement" => {
            p.parent().map(|gp| gp.kind() == "program").unwrap_or(true)
        }
        _ => false,
    }
}

/// `const X = (...) => …` — any declarator whose value is an arrow function.
fn declares_arrow_function(node: tree_sitter::Node) -> bool {
    let mut cursor = node.walk();
    let found = node.children(&mut cursor).any(|d| {
        d.kind() == "variable_declarator"
            && d.child_by_field_name("value")
                .map(|v| v.kind() == "arrow_function")
                .unwrap_or(false)
    });
    found
}

fn count_ts_types(node: tree_sitter::Node, source: &str, counts: &mut TypeCounts) {
    match node.kind() {
        "interface_declaration" => counts.abstract_count += 1,
        "abstract_class_declaration" => counts.abstract_count += 1,
        "class_declaration" => counts.concrete_count += 1,
        "enum_declaration" => counts.concrete_count += 1,
        // Module-level functions — declared or arrow — are implementation
        // (#413): a React component file is not "100% abstract" because it
        // declares a Props interface. Nested helpers are not counted.
        "function_declaration" if is_module_level(node) => counts.concrete_count += 1,
        "lexical_declaration" if is_module_level(node) && declares_arrow_function(node) => {
            counts.concrete_count += 1
        }
        _ => {}
    }
    let _ = source;
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        count_ts_types(child, source, counts);
    }
}

fn parse_with_ts_grammar(source: &str, tsx: bool) -> Vec<ImportEntry> {
    let mut parser = Parser::new();
    let lang: tree_sitter::Language = if tsx {
        tree_sitter_typescript::LANGUAGE_TSX.into()
    } else {
        tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into()
    };
    parser
        .set_language(&lang)
        .expect("Failed to set TypeScript language");

    let tree = match parser.parse(source, None) {
        Some(t) => t,
        None => return Vec::new(),
    };

    let mut imports = Vec::new();
    collect_typescript_imports(tree.root_node(), source, &mut imports);
    imports
}

fn collect_typescript_imports(
    node: tree_sitter::Node,
    source: &str,
    imports: &mut Vec<ImportEntry>,
) {
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
        collect_typescript_imports(child, source, imports);
    }
}

fn node_text(node: tree_sitter::Node, source: &str) -> String {
    source[node.byte_range()].to_string()
}

// ── Resolver ──────────────────────────────────────────────────────────────────

pub(super) fn resolve_typescript_import(
    import_path: &str,
    source_file: &str,
    known_paths: &[String],
) -> Option<String> {
    if !import_path.starts_with('.') {
        return None;
    }

    let source_dir = Path::new(source_file).parent()?;
    let resolved = source_dir.join(import_path);

    let mut components: Vec<String> = Vec::new();
    for comp in resolved.components() {
        match comp {
            std::path::Component::ParentDir => {
                components.pop();
            }
            std::path::Component::CurDir => {}
            std::path::Component::Normal(s) => components.push(s.to_string_lossy().to_string()),
            _ => {}
        }
    }
    let base = components.join("/");

    for ext in &["ts", "tsx", "js", "jsx"] {
        let candidate = format!("{}.{}", base, ext);
        if known_paths.contains(&candidate) {
            return Some(candidate);
        }
    }

    for ext in &["ts", "tsx", "js", "jsx"] {
        let candidate = format!("{}/index.{}", base, ext);
        if known_paths.contains(&candidate) {
            return Some(candidate);
        }
    }

    None
}

// ── Tests ──────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn ts_paths() -> Vec<String> {
        vec![
            "src/components/Button.tsx".to_string(),
            "src/components/Input.tsx".to_string(),
            "src/pages/Home.ts".to_string(),
            "src/utils/helpers.ts".to_string(),
            "src/shared/index.ts".to_string(),
        ]
    }

    #[test]
    fn ts_parses_simple_import() {
        let source = "import { Component } from './component';";
        let imports = TypeScriptParser.parse(source);
        assert_eq!(imports.len(), 1);
        assert_eq!(imports[0].path, "./component");
        assert_eq!(imports[0].line_number, 1);
    }

    #[test]
    fn ts_parses_default_import() {
        let source = "import React from 'react';";
        let imports = TypeScriptParser.parse(source);
        assert_eq!(imports.len(), 1);
        assert_eq!(imports[0].path, "react");
    }

    #[test]
    fn ts_parses_namespace_import() {
        let source = "import * as utils from '../utils';";
        let imports = TypeScriptParser.parse(source);
        assert_eq!(imports.len(), 1);
        assert_eq!(imports[0].path, "../utils");
    }

    #[test]
    fn ts_parses_multiple_imports() {
        let source = "import { Foo } from './foo';\nimport { Bar } from './bar';\nimport { Baz } from '../baz';\n";
        let imports = TypeScriptParser.parse(source);
        assert_eq!(imports.len(), 3);
        assert_eq!(imports[0].line_number, 1);
        assert_eq!(imports[1].line_number, 2);
        assert_eq!(imports[2].line_number, 3);
    }

    #[test]
    fn ts_parses_relative_path() {
        let source = "import { helper } from '../../shared/helper';";
        let imports = TypeScriptParser.parse(source);
        assert_eq!(imports.len(), 1);
        assert_eq!(imports[0].path, "../../shared/helper");
    }

    #[test]
    fn ts_handles_empty_source() {
        let imports = TypeScriptParser.parse("");
        assert!(imports.is_empty());
    }

    #[test]
    fn ts_ignores_non_import_code() {
        let source = "const x = 42;\nfunction hello() {}";
        let imports = TypeScriptParser.parse(source);
        assert!(imports.is_empty());
    }

    #[test]
    fn tsx_parses_imports() {
        let source = "import { useState } from 'react';\nimport { Button } from './Button';";
        let imports = TsxParser.parse(source);
        assert_eq!(imports.len(), 2);
        assert_eq!(imports[0].path, "react");
        assert_eq!(imports[1].path, "./Button");
    }

    #[test]
    fn ts_resolves_relative_import() {
        let paths = ts_paths();
        let result = TypeScriptParser.resolve("./helpers", "src/utils/helpers.ts", &paths);
        assert_eq!(result, Some("src/utils/helpers.ts".to_string()));
    }

    #[test]
    fn ts_counts_interface_abstract_class_and_concrete_class() {
        let source = "interface I {}\nabstract class A {}\nclass C {}\nenum E {}\n";
        let counts = TypeScriptParser.count_type_declarations(source);
        assert_eq!(counts.abstract_count, 2, "got {:?}", counts);
        assert_eq!(counts.concrete_count, 2, "got {:?}", counts);
    }

    /// A React component module — a Props interface plus exported functions
    /// — is implementation, not abstraction (#413). Nested helpers inside a
    /// function body are not counted; type aliases are neither.
    #[test]
    fn ts_counts_exported_and_top_level_functions_as_concrete() {
        let source = "interface Props { a: number }\ntype Mode = 'a' | 'b';\nexport function Card(p: Props) { const inner = () => 1; return inner(); }\nexport const Row = (p: Props) => null;\nfunction helper() {}\nconst n = 3;\n";
        let counts = TsxParser.count_type_declarations(source);
        assert_eq!(counts.abstract_count, 1, "{counts:?}");
        assert_eq!(counts.concrete_count, 3, "Card, Row, helper: {counts:?}");
    }

    #[test]
    fn tsx_counts_match_ts_counts() {
        let source = "interface I {}\nabstract class A {}\nclass C {}\n";
        let counts = TsxParser.count_type_declarations(source);
        assert_eq!(counts.abstract_count, 2);
        assert_eq!(counts.concrete_count, 1);
    }

    #[test]
    fn ts_resolves_sibling_import() {
        let paths = ts_paths();
        let result = TypeScriptParser.resolve("../utils/helpers", "src/pages/Home.ts", &paths);
        assert_eq!(result, Some("src/utils/helpers.ts".to_string()));
    }

    #[test]
    fn ts_resolves_index_file() {
        let paths = ts_paths();
        let result = TypeScriptParser.resolve("../shared", "src/pages/Home.ts", &paths);
        assert_eq!(result, Some("src/shared/index.ts".to_string()));
    }

    #[test]
    fn ts_returns_none_for_npm_package() {
        let paths = ts_paths();
        let result = TypeScriptParser.resolve("react", "src/pages/Home.ts", &paths);
        assert!(result.is_none());
    }

    #[test]
    fn ts_resolves_tsx_extension() {
        let paths = ts_paths();
        let result = TypeScriptParser.resolve("../components/Button", "src/pages/Home.ts", &paths);
        assert_eq!(result, Some("src/components/Button.tsx".to_string()));
    }
}
