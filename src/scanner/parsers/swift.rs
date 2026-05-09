use tree_sitter::Parser;

use super::{ImportEntry, LanguageParser};

pub struct SwiftParser;

impl LanguageParser for SwiftParser {
    fn parse(&self, source: &str) -> Vec<ImportEntry> {
        let mut parser = Parser::new();
        let swift_lang: tree_sitter::Language = tree_sitter_swift::LANGUAGE.into();
        parser
            .set_language(&swift_lang)
            .expect("Failed to set Swift language");

        let tree = match parser.parse(source, None) {
            Some(t) => t,
            None => return Vec::new(),
        };

        let mut imports = Vec::new();
        collect_swift_imports(tree.root_node(), source, &mut imports);
        imports
    }

    fn resolve(
        &self,
        import_path: &str,
        _source_file: &str,
        known_paths: &[String],
    ) -> Option<String> {
        let filename = format!("{}.swift", import_path);
        known_paths.iter().find(|p| p.ends_with(&filename)).cloned()
    }
}

fn collect_swift_imports(node: tree_sitter::Node, source: &str, imports: &mut Vec<ImportEntry>) {
    if node.kind() == "import_declaration" {
        let line_number = (node.start_position().row + 1) as i32;
        let full = node_text(node, source);
        let path = full.trim_start_matches("import ").trim().to_string();
        if !path.is_empty() {
            imports.push(ImportEntry { path, line_number });
        }
        return;
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_swift_imports(child, source, imports);
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
    fn swift_parses_simple_import() {
        let source = "import Foundation";
        let imports = SwiftParser.parse(source);
        assert_eq!(imports.len(), 1);
        assert_eq!(imports[0].path, "Foundation");
        assert_eq!(imports[0].line_number, 1);
    }

    #[test]
    fn swift_parses_multiple_imports() {
        let source = "import UIKit\nimport SwiftUI\nimport Combine\n";
        let imports = SwiftParser.parse(source);
        assert_eq!(imports.len(), 3);
        assert_eq!(imports[0].path, "UIKit");
        assert_eq!(imports[1].path, "SwiftUI");
        assert_eq!(imports[2].path, "Combine");
    }

    #[test]
    fn swift_handles_empty_source() {
        let imports = SwiftParser.parse("");
        assert!(imports.is_empty());
    }

    #[test]
    fn swift_ignores_non_import_code() {
        let source = "func hello() { print(\"hello\") }";
        let imports = SwiftParser.parse(source);
        assert!(imports.is_empty());
    }
}
