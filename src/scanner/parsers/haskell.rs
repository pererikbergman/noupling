use tree_sitter::Parser;

use super::{ImportEntry, LanguageParser};

pub struct HaskellParser;

impl LanguageParser for HaskellParser {
    fn parse(&self, source: &str) -> Vec<ImportEntry> {
        let mut parser = Parser::new();
        let hs_lang: tree_sitter::Language = tree_sitter_haskell::LANGUAGE.into();
        parser
            .set_language(&hs_lang)
            .expect("Failed to set Haskell language");

        let tree = match parser.parse(source, None) {
            Some(t) => t,
            None => return Vec::new(),
        };

        let mut imports = Vec::new();
        collect_haskell_imports(tree.root_node(), source, &mut imports);
        imports
    }

    fn resolve(
        &self,
        import_path: &str,
        _source_file: &str,
        known_paths: &[String],
    ) -> Option<String> {
        let file_path = import_path.replace('.', "/");
        let candidate = format!("{}.hs", file_path);
        known_paths
            .iter()
            .find(|p| p.ends_with(&candidate))
            .cloned()
    }
}

fn collect_haskell_imports(
    node: tree_sitter::Node,
    source: &str,
    imports: &mut Vec<ImportEntry>,
) {
    if node.kind() == "import" {
        let line_number = (node.start_position().row + 1) as i32;
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "module" {
                let text = node_text(child, source);
                imports.push(ImportEntry {
                    path: text,
                    line_number,
                });
                return;
            }
        }
        // Fallback: parse from text
        let full = node_text(node, source);
        let path = full
            .trim_start_matches("import ")
            .trim_start_matches("qualified ")
            .split_whitespace()
            .next()
            .unwrap_or("")
            .to_string();
        if !path.is_empty() {
            imports.push(ImportEntry { path, line_number });
        }
        return;
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_haskell_imports(child, source, imports);
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
    fn haskell_parses_import() {
        let source = "import Data.List";
        let imports = HaskellParser.parse(source);
        assert_eq!(imports.len(), 1);
        assert_eq!(imports[0].path, "Data.List");
    }

    #[test]
    fn haskell_parses_multiple_imports() {
        let source = "import Data.Map\nimport Control.Monad\nimport System.IO\n";
        let imports = HaskellParser.parse(source);
        assert_eq!(imports.len(), 3);
    }

    #[test]
    fn haskell_handles_empty_source() {
        let imports = HaskellParser.parse("");
        assert!(imports.is_empty());
    }
}
