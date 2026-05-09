use tree_sitter::Parser;

use super::{ImportEntry, LanguageParser};

pub struct GoParser;

impl LanguageParser for GoParser {
    fn parse(&self, source: &str) -> Vec<ImportEntry> {
        let mut parser = Parser::new();
        let go_lang: tree_sitter::Language = tree_sitter_go::LANGUAGE.into();
        parser
            .set_language(&go_lang)
            .expect("Failed to set Go language");

        let tree = match parser.parse(source, None) {
            Some(t) => t,
            None => return Vec::new(),
        };

        let mut imports = Vec::new();
        collect_go_imports(tree.root_node(), source, &mut imports);
        imports
    }

    fn resolve(
        &self,
        import_path: &str,
        _source_file: &str,
        known_paths: &[String],
    ) -> Option<String> {
        resolve_go_import(import_path, known_paths)
    }
}

fn collect_go_imports(node: tree_sitter::Node, source: &str, imports: &mut Vec<ImportEntry>) {
    match node.kind() {
        "import_spec" => {
            let line_number = (node.start_position().row + 1) as i32;
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                if child.kind() == "interpreted_string_literal" {
                    let text = node_text(child, source);
                    let path = text.trim_matches('"').to_string();
                    if !path.is_empty() {
                        imports.push(ImportEntry { path, line_number });
                    }
                    return;
                }
            }
        }
        _ => {
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                collect_go_imports(child, source, imports);
            }
        }
    }
}

fn node_text(node: tree_sitter::Node, source: &str) -> String {
    source[node.byte_range()].to_string()
}

// ── Resolver ──────────────────────────────────────────────────────────────────

fn resolve_go_import(import_path: &str, known_paths: &[String]) -> Option<String> {
    let dir_suffix = format!("/{}/", import_path);
    if let Some(found) = known_paths
        .iter()
        .find(|p| p.contains(&dir_suffix) && p.ends_with(".go"))
    {
        return Some(found.clone());
    }
    let candidate = format!("{}.go", import_path);
    if let Some(found) = known_paths.iter().find(|p| p.ends_with(&candidate)) {
        return Some(found.clone());
    }
    None
}

// ── Tests ──────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn go_parses_single_import() {
        let source = "package main\n\nimport \"fmt\"";
        let imports = GoParser.parse(source);
        assert_eq!(imports.len(), 1);
        assert_eq!(imports[0].path, "fmt");
    }

    #[test]
    fn go_parses_grouped_imports() {
        let source = "package main\n\nimport (\n\t\"fmt\"\n\t\"os\"\n\t\"myapp/utils\"\n)";
        let imports = GoParser.parse(source);
        assert_eq!(imports.len(), 3);
        assert_eq!(imports[0].path, "fmt");
        assert_eq!(imports[1].path, "os");
        assert_eq!(imports[2].path, "myapp/utils");
    }

    #[test]
    fn go_handles_empty_source() {
        let imports = GoParser.parse("");
        assert!(imports.is_empty());
    }
}
