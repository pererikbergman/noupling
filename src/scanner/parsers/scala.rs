use tree_sitter::Parser;

use super::{ImportEntry, LanguageParser};

pub struct ScalaParser;

impl LanguageParser for ScalaParser {
    fn parse(&self, source: &str) -> Vec<ImportEntry> {
        let mut parser = Parser::new();
        let scala_lang: tree_sitter::Language = tree_sitter_scala::LANGUAGE.into();
        parser
            .set_language(&scala_lang)
            .expect("Failed to set Scala language");

        let tree = match parser.parse(source, None) {
            Some(t) => t,
            None => return Vec::new(),
        };

        let mut imports = Vec::new();
        collect_scala_imports(tree.root_node(), source, &mut imports);
        imports
    }

    fn resolve(
        &self,
        import_path: &str,
        _source_file: &str,
        known_paths: &[String],
    ) -> Option<String> {
        resolve_scala_import(import_path, known_paths)
    }
}

fn collect_scala_imports(node: tree_sitter::Node, source: &str, imports: &mut Vec<ImportEntry>) {
    if node.kind() == "import_declaration" {
        let line_number = (node.start_position().row + 1) as i32;
        // The full import text, e.g. "import scala.collection.mutable"
        let full = node_text(node, source);
        let path = full.trim_start_matches("import").trim().to_string();
        if !path.is_empty() {
            imports.push(ImportEntry { path, line_number });
        }
        return;
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_scala_imports(child, source, imports);
    }
}

fn node_text(node: tree_sitter::Node, source: &str) -> String {
    source[node.byte_range()].to_string()
}

// ── Resolver ──────────────────────────────────────────────────────────────────

/// Scala imports use dot-separated package paths (e.g. `com.example.bar.Baz`).
/// Convert to a slash path and look for `.scala` / `.sc` files.
fn resolve_scala_import(import_path: &str, known_paths: &[String]) -> Option<String> {
    // Strip wildcard suffix if present (e.g. `com.example._` or `com.example.{Baz, Qux}`)
    let base = import_path
        .trim_end_matches('_')
        .trim_end_matches('.')
        .split('{')
        .next()
        .unwrap_or(import_path)
        .trim_end_matches('.')
        .trim();

    let file_path = base.replace('.', "/");

    for ext in &["scala", "sc"] {
        let candidate = format!("{}.{}", file_path, ext);
        if let Some(found) = known_paths.iter().find(|p| p.ends_with(&candidate)) {
            return Some(found.clone());
        }
    }

    // Also try the last segment as a class name: com/example/bar/Baz.scala
    // → already handled above since we use ends_with.
    None
}

// ── Tests ──────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(source: &str) -> Vec<ImportEntry> {
        ScalaParser.parse(source)
    }

    #[test]
    fn scala_parses_simple_import() {
        let source = "import scala.collection.mutable\n";
        let imports = parse(source);
        assert_eq!(imports.len(), 1);
        assert_eq!(imports[0].path, "scala.collection.mutable");
        assert_eq!(imports[0].line_number, 1);
    }

    #[test]
    fn scala_handles_empty_source() {
        let imports = parse("");
        assert!(imports.is_empty());
    }

    #[test]
    fn scala_parses_multiple_imports() {
        let source =
            "import scala.collection.mutable\nimport com.example.bar.Baz\nimport com.example.helper._\n";
        let imports = parse(source);
        assert_eq!(imports.len(), 3);
        assert_eq!(imports[0].path, "scala.collection.mutable");
        assert_eq!(imports[1].path, "com.example.bar.Baz");
        assert_eq!(imports[2].path, "com.example.helper._");
    }

    #[test]
    fn scala_parses_grouped_import() {
        let source = "import com.example.bar.{Baz, Qux}\n";
        let imports = parse(source);
        assert_eq!(imports.len(), 1);
        assert!(imports[0].path.starts_with("com.example.bar."));
    }

    #[test]
    fn scala_line_numbers_correct() {
        let source = "package com.example\n\nimport com.example.Foo\nimport com.example.Bar\n";
        let imports = parse(source);
        assert_eq!(imports.len(), 2);
        assert_eq!(imports[0].line_number, 3);
        assert_eq!(imports[1].line_number, 4);
    }

    #[test]
    fn scala_resolver_converts_package_to_path() {
        let known = vec!["src/main/scala/com/example/bar/Baz.scala".to_string()];
        let result = ScalaParser.resolve("com.example.bar.Baz", "", &known);
        assert_eq!(
            result,
            Some("src/main/scala/com/example/bar/Baz.scala".to_string())
        );
    }

    #[test]
    fn scala_resolver_returns_none_for_external() {
        let known = vec!["src/main/scala/com/example/bar/Baz.scala".to_string()];
        let result = ScalaParser.resolve("scala.collection.mutable", "", &known);
        assert!(result.is_none());
    }
}
