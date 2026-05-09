use tree_sitter::Parser;

use super::{ImportEntry, LanguageParser};

pub struct JavaParser;

impl LanguageParser for JavaParser {
    fn parse(&self, source: &str) -> Vec<ImportEntry> {
        let mut parser = Parser::new();
        let java_lang: tree_sitter::Language = tree_sitter_java::LANGUAGE.into();
        parser
            .set_language(&java_lang)
            .expect("Failed to set Java language");

        let tree = match parser.parse(source, None) {
            Some(t) => t,
            None => return Vec::new(),
        };

        let mut imports = Vec::new();
        collect_java_imports(tree.root_node(), source, &mut imports);
        imports
    }

    fn resolve(
        &self,
        import_path: &str,
        _source_file: &str,
        known_paths: &[String],
    ) -> Option<String> {
        resolve_java_import(import_path, known_paths)
    }
}

fn collect_java_imports(node: tree_sitter::Node, source: &str, imports: &mut Vec<ImportEntry>) {
    if node.kind() == "import_declaration" {
        let line_number = (node.start_position().row + 1) as i32;
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "scoped_identifier" {
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
            .trim_start_matches("import ")
            .trim_start_matches("static ")
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
        collect_java_imports(child, source, imports);
    }
}

fn node_text(node: tree_sitter::Node, source: &str) -> String {
    source[node.byte_range()].to_string()
}

// ── Resolver ──────────────────────────────────────────────────────────────────

fn resolve_java_import(import_path: &str, known_paths: &[String]) -> Option<String> {
    let segments: Vec<&str> = import_path.split('.').collect();
    if segments.is_empty() {
        return None;
    }

    let file_path = segments.join("/");
    let candidate = format!("{}.java", file_path);
    if let Some(found) = known_paths.iter().find(|p| p.ends_with(&candidate)) {
        return Some(found.clone());
    }

    if segments.len() > 1 {
        let parent = segments[..segments.len() - 1].join("/");
        let candidate = format!("{}.java", parent);
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
    fn java_parses_import() {
        let source = "import com.example.MyClass;";
        let imports = JavaParser.parse(source);
        assert_eq!(imports.len(), 1);
        assert_eq!(imports[0].path, "com.example.MyClass");
    }

    #[test]
    fn java_parses_multiple_imports() {
        let source = "import java.util.List;\nimport java.util.Map;\nimport com.example.Foo;\n";
        let imports = JavaParser.parse(source);
        assert_eq!(imports.len(), 3);
    }

    #[test]
    fn java_handles_empty_source() {
        let imports = JavaParser.parse("");
        assert!(imports.is_empty());
    }
}
