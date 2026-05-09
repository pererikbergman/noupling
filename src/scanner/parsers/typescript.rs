use std::path::Path;
use tree_sitter::Parser;

use super::{ImportEntry, LanguageParser};

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
            std::path::Component::Normal(s) => {
                components.push(s.to_string_lossy().to_string())
            }
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
        let source =
            "import { useState } from 'react';\nimport { Button } from './Button';";
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
        let result =
            TypeScriptParser.resolve("../components/Button", "src/pages/Home.ts", &paths);
        assert_eq!(result, Some("src/components/Button.tsx".to_string()));
    }
}
