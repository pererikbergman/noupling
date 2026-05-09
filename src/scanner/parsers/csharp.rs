use tree_sitter::Parser;

use super::{ImportEntry, LanguageParser};

pub struct CSharpParser;

impl LanguageParser for CSharpParser {
    fn parse(&self, source: &str) -> Vec<ImportEntry> {
        let mut parser = Parser::new();
        let csharp_lang: tree_sitter::Language = tree_sitter_c_sharp::LANGUAGE.into();
        parser
            .set_language(&csharp_lang)
            .expect("Failed to set C# language");

        let tree = match parser.parse(source, None) {
            Some(t) => t,
            None => return Vec::new(),
        };

        let mut imports = Vec::new();
        collect_csharp_imports(tree.root_node(), source, &mut imports);
        imports
    }

    fn resolve(
        &self,
        import_path: &str,
        _source_file: &str,
        known_paths: &[String],
    ) -> Option<String> {
        resolve_csharp_import(import_path, known_paths)
    }
}

fn collect_csharp_imports(node: tree_sitter::Node, source: &str, imports: &mut Vec<ImportEntry>) {
    if node.kind() == "using_directive" {
        let line_number = (node.start_position().row + 1) as i32;
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "qualified_name" || child.kind() == "identifier" {
                let text = node_text(child, source);
                imports.push(ImportEntry {
                    path: text,
                    line_number,
                });
                return;
            }
        }
        // Fallback
        let full = node_text(node, source);
        let path = full
            .trim_start_matches("using ")
            .trim_end_matches(';')
            .trim()
            .to_string();
        if !path.is_empty() {
            imports.push(ImportEntry { path, line_number });
        }
        return;
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_csharp_imports(child, source, imports);
    }
}

fn node_text(node: tree_sitter::Node, source: &str) -> String {
    source[node.byte_range()].to_string()
}

// ── Resolver ──────────────────────────────────────────────────────────────────

fn resolve_csharp_import(import_path: &str, known_paths: &[String]) -> Option<String> {
    let segments: Vec<&str> = import_path.split('.').collect();
    if segments.is_empty() {
        return None;
    }

    let file_path = segments.join("/");
    let candidate = format!("{}.cs", file_path);
    if let Some(found) = known_paths.iter().find(|p| p.ends_with(&candidate)) {
        return Some(found.clone());
    }

    if segments.len() > 1 {
        let parent_path = segments[..segments.len() - 1].join("/");
        let candidate = format!("{}.cs", parent_path);
        if let Some(found) = known_paths.iter().find(|p| p.ends_with(&candidate)) {
            return Some(found.clone());
        }
    }

    None
}

// ── Tests ──────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn csharp_parses_simple_using() {
        let source = "using System;";
        let imports = CSharpParser.parse(source);
        assert_eq!(imports.len(), 1);
        assert_eq!(imports[0].path, "System");
        assert_eq!(imports[0].line_number, 1);
    }

    #[test]
    fn csharp_parses_qualified_using() {
        let source = "using System.Collections.Generic;";
        let imports = CSharpParser.parse(source);
        assert_eq!(imports.len(), 1);
        assert_eq!(imports[0].path, "System.Collections.Generic");
    }

    #[test]
    fn csharp_parses_multiple_usings() {
        let source = "using System;\nusing System.Linq;\nusing MyApp.Data;\n";
        let imports = CSharpParser.parse(source);
        assert_eq!(imports.len(), 3);
        assert_eq!(imports[0].path, "System");
        assert_eq!(imports[1].path, "System.Linq");
        assert_eq!(imports[2].path, "MyApp.Data");
    }

    #[test]
    fn csharp_handles_empty_source() {
        let imports = CSharpParser.parse("");
        assert!(imports.is_empty());
    }

    #[test]
    fn csharp_ignores_non_using_code() {
        let source = "namespace MyApp { class Foo {} }";
        let imports = CSharpParser.parse(source);
        assert!(imports.is_empty());
    }
}
