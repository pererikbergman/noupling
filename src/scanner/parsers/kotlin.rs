use tree_sitter::Parser;

use super::{ImportEntry, LanguageParser};

pub struct KotlinParser;

impl LanguageParser for KotlinParser {
    fn parse(&self, source: &str) -> Vec<ImportEntry> {
        let mut parser = Parser::new();
        let kotlin_lang: tree_sitter::Language = tree_sitter_kotlin_ng::LANGUAGE.into();
        parser
            .set_language(&kotlin_lang)
            .expect("Failed to set Kotlin language");

        let tree = match parser.parse(source, None) {
            Some(t) => t,
            None => return Vec::new(),
        };

        let mut imports = Vec::new();
        collect_kotlin_imports(tree.root_node(), source, &mut imports);
        imports
    }

    fn resolve(
        &self,
        import_path: &str,
        _source_file: &str,
        known_paths: &[String],
    ) -> Option<String> {
        resolve_kotlin_import(import_path, known_paths)
    }
}

fn collect_kotlin_imports(node: tree_sitter::Node, source: &str, imports: &mut Vec<ImportEntry>) {
    if node.kind() == "import" {
        let line_number = (node.start_position().row + 1) as i32;
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "qualified_identifier" {
                let text = node_text(child, source);
                imports.push(ImportEntry {
                    path: text,
                    line_number,
                });
                return;
            }
        }
        // Fallback: extract from full text
        let full = node_text(node, source);
        let path = full.trim_start_matches("import ").trim().to_string();
        if !path.is_empty() {
            imports.push(ImportEntry { path, line_number });
        }
        return;
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_kotlin_imports(child, source, imports);
    }
}

fn node_text(node: tree_sitter::Node, source: &str) -> String {
    source[node.byte_range()].to_string()
}

// ── Resolver ──────────────────────────────────────────────────────────────────

fn resolve_kotlin_import(import_path: &str, known_paths: &[String]) -> Option<String> {
    let segments: Vec<&str> = import_path.split('.').collect();
    if segments.is_empty() {
        return None;
    }

    let file_path = segments.join("/");
    for ext in &["kt", "kts"] {
        let candidate = format!("{}.{}", file_path, ext);
        if let Some(found) = known_paths.iter().find(|p| p.ends_with(&candidate)) {
            return Some(found.clone());
        }
    }

    if segments.len() > 1 {
        let parent_path = segments[..segments.len() - 1].join("/");
        for ext in &["kt", "kts"] {
            let candidate = format!("{}.{}", parent_path, ext);
            if let Some(found) = known_paths.iter().find(|p| p.ends_with(&candidate)) {
                return Some(found.clone());
            }
        }
    }

    None
}

// ── Tests ──────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(source: &str) -> Vec<ImportEntry> {
        KotlinParser.parse(source)
    }

    fn resolve(import_path: &str, known_paths: &[String]) -> Option<String> {
        KotlinParser.resolve(import_path, "", known_paths)
    }

    fn kotlin_paths() -> Vec<String> {
        vec![
            "app/src/main/kotlin/com/example/MainActivity.kt".to_string(),
            "app/src/main/kotlin/com/example/data/Repository.kt".to_string(),
            "app/src/main/kotlin/com/example/data/Model.kt".to_string(),
            "app/src/main/kotlin/com/example/ui/HomeScreen.kt".to_string(),
        ]
    }

    #[test]
    fn kotlin_parses_simple_import() {
        let source = "import com.example.MyClass";
        let imports = parse(source);
        assert_eq!(imports.len(), 1);
        assert_eq!(imports[0].path, "com.example.MyClass");
        assert_eq!(imports[0].line_number, 1);
    }

    #[test]
    fn kotlin_parses_multiple_imports() {
        let source = "import com.example.Foo\nimport com.example.Bar\nimport org.utils.Helper\n";
        let imports = parse(source);
        assert_eq!(imports.len(), 3);
        assert_eq!(imports[0].path, "com.example.Foo");
        assert_eq!(imports[1].path, "com.example.Bar");
        assert_eq!(imports[2].path, "org.utils.Helper");
    }

    #[test]
    fn kotlin_parses_wildcard_import() {
        let source = "import com.example.*";
        let imports = parse(source);
        assert_eq!(imports.len(), 1);
        assert!(imports[0].path.contains("com.example"));
    }

    #[test]
    fn kotlin_line_numbers_correct() {
        let source = "package com.example\n\nimport com.example.Foo\nimport com.example.Bar\n";
        let imports = parse(source);
        assert_eq!(imports.len(), 2);
        assert_eq!(imports[0].line_number, 3);
        assert_eq!(imports[1].line_number, 4);
    }

    #[test]
    fn kotlin_handles_empty_source() {
        let imports = parse("");
        assert!(imports.is_empty());
    }

    #[test]
    fn kotlin_ignores_non_import_code() {
        let source = "fun main() { println(\"hello\") }";
        let imports = parse(source);
        assert!(imports.is_empty());
    }

    #[test]
    fn kotlin_resolves_direct_import() {
        let paths = kotlin_paths();
        let result = resolve("com.example.data.Repository", &paths);
        assert_eq!(
            result,
            Some("app/src/main/kotlin/com/example/data/Repository.kt".to_string())
        );
    }

    #[test]
    fn kotlin_resolves_class_to_file() {
        let paths = kotlin_paths();
        let result = resolve("com.example.ui.HomeScreen", &paths);
        assert_eq!(
            result,
            Some("app/src/main/kotlin/com/example/ui/HomeScreen.kt".to_string())
        );
    }

    #[test]
    fn kotlin_returns_none_for_external_dep() {
        let paths = kotlin_paths();
        let result = resolve("androidx.compose.runtime.Composable", &paths);
        assert!(result.is_none());
    }
}
