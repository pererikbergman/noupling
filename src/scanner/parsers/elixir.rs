use tree_sitter::Parser;

use super::{ImportEntry, LanguageParser};

pub struct ElixirParser;

impl LanguageParser for ElixirParser {
    fn parse(&self, source: &str) -> Vec<ImportEntry> {
        let mut parser = Parser::new();
        let elixir_lang: tree_sitter::Language = tree_sitter_elixir::LANGUAGE.into();
        parser
            .set_language(&elixir_lang)
            .expect("Failed to set Elixir language");

        let tree = match parser.parse(source, None) {
            Some(t) => t,
            None => return Vec::new(),
        };

        let mut imports = Vec::new();
        collect_elixir_imports(tree.root_node(), source, &mut imports);
        imports
    }

    fn resolve(
        &self,
        import_path: &str,
        _source_file: &str,
        known_paths: &[String],
    ) -> Option<String> {
        resolve_elixir_import(import_path, known_paths)
    }
}

/// Elixir import directives: alias, import, use, require.
///
/// These all appear as `call` nodes in the tree-sitter-elixir grammar
/// where the function identifier is one of the four keywords.
fn collect_elixir_imports(node: tree_sitter::Node, source: &str, imports: &mut Vec<ImportEntry>) {
    if node.kind() == "call" {
        let mut cursor = node.walk();
        let children: Vec<_> = node.children(&mut cursor).collect();

        // The first child of a call node is the function identifier.
        if let Some(first) = children.first() {
            let keyword = node_text(*first, source);
            if matches!(keyword.as_str(), "alias" | "import" | "use" | "require") {
                let line_number = (node.start_position().row + 1) as i32;

                // Collect the argument (second child after keyword, skipping parens if present).
                for child in children.iter().skip(1) {
                    let kind = child.kind();
                    match kind {
                        // alias MyApp.Bar  → alias_node or dot/qualified name
                        "alias" | "atom" => {
                            let text = node_text(*child, source);
                            push_import(&text, line_number, imports);
                            return;
                        }
                        // The module path is usually an `arguments` node.
                        "arguments" => {
                            collect_from_arguments(*child, source, line_number, imports);
                            return;
                        }
                        _ => {}
                    }
                }

                // Fallback: extract the full call text and strip the keyword.
                let full = node_text(node, source);
                if let Some(rest) = full.strip_prefix(&keyword) {
                    let rest = rest.trim_start_matches('(').trim();
                    let path = rest
                        .trim_end_matches(')')
                        .split([',', '\n'])
                        .next()
                        .unwrap_or("")
                        .trim()
                        .to_string();
                    push_import(&path, line_number, imports);
                }
                return;
            }
        }
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_elixir_imports(child, source, imports);
    }
}

/// Drill into an `arguments` node and push each module reference found.
fn collect_from_arguments(
    node: tree_sitter::Node,
    source: &str,
    line_number: i32,
    imports: &mut Vec<ImportEntry>,
) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            // alias MyApp.{Baz, Qux}  → the `{...}` part
            "tuple" | "list" => {
                // Reconstruct the base from sibling before this node if any.
                // For now, just record the raw text.
                let text = node_text(child, source);
                push_import(&text, line_number, imports);
            }
            // Dot-separated module name, e.g. MyApp.Bar
            "dot" | "alias" | "atom" | "identifier" => {
                let text = node_text(child, source);
                push_import(&text, line_number, imports);
            }
            _ => {}
        }
    }
}

fn push_import(raw: &str, line_number: i32, imports: &mut Vec<ImportEntry>) {
    let cleaned = raw.trim().trim_matches('"').trim_matches('\'').to_string();
    if !cleaned.is_empty() && cleaned != "," {
        imports.push(ImportEntry {
            path: cleaned,
            line_number,
        });
    }
}

fn node_text(node: tree_sitter::Node, source: &str) -> String {
    source[node.byte_range()].to_string()
}

// ── Resolver ──────────────────────────────────────────────────────────────────

/// Elixir modules use dot-separated names (e.g. `MyApp.Bar`).
/// Convert `MyApp.Bar` → `my_app/bar.ex` and look for it in known paths.
fn resolve_elixir_import(import_path: &str, known_paths: &[String]) -> Option<String> {
    // Strip leading `alias`/`use`/`import`/`require` if they leaked through.
    let path = import_path
        .trim_start_matches("alias ")
        .trim_start_matches("import ")
        .trim_start_matches("use ")
        .trim_start_matches("require ")
        .trim();

    // Convert CamelCase dot path → snake_case slash path.
    let file_path = path
        .split('.')
        .map(camel_to_snake)
        .collect::<Vec<_>>()
        .join("/");

    for ext in &["ex", "exs"] {
        let candidate = format!("{}.{}", file_path, ext);
        if let Some(found) = known_paths.iter().find(|p| p.ends_with(&candidate)) {
            return Some(found.clone());
        }
    }
    None
}

fn camel_to_snake(s: &str) -> String {
    let mut result = String::new();
    for (i, c) in s.chars().enumerate() {
        if c.is_uppercase() && i > 0 {
            result.push('_');
        }
        result.push(c.to_lowercase().next().unwrap_or(c));
    }
    result
}

// ── Tests ──────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(source: &str) -> Vec<ImportEntry> {
        ElixirParser.parse(source)
    }

    #[test]
    fn elixir_parses_simple_alias() {
        let source = "alias MyApp.Bar\n";
        let imports = parse(source);
        assert!(!imports.is_empty(), "expected at least one import");
        assert!(
            imports
                .iter()
                .any(|i| i.path.contains("MyApp") || i.path.contains("Bar")),
            "expected module reference, got: {:?}",
            imports.iter().map(|i| &i.path).collect::<Vec<_>>()
        );
    }

    #[test]
    fn elixir_handles_empty_source() {
        let imports = parse("");
        assert!(imports.is_empty());
    }

    #[test]
    fn elixir_parses_multiple_directives() {
        let source = indoc(
            "defmodule MyApp.Foo do
  alias MyApp.Bar
  import MyApp.Helper
  use GenServer
  require Logger
end
",
        );
        let imports = parse(&source);
        // We expect at least 4 directives (alias, import, use, require)
        assert!(
            imports.len() >= 4,
            "expected >= 4 imports, got {}: {:?}",
            imports.len(),
            imports.iter().map(|i| &i.path).collect::<Vec<_>>()
        );
    }

    #[test]
    fn elixir_line_numbers_correct() {
        let source = "alias MyApp.Bar\nimport MyApp.Helper\n";
        let imports = parse(source);
        assert!(!imports.is_empty());
        assert_eq!(imports[0].line_number, 1);
    }

    #[test]
    fn elixir_resolver_converts_module_to_path() {
        let known = vec!["lib/my_app/bar.ex".to_string()];
        let result = ElixirParser.resolve("MyApp.Bar", "", &known);
        assert_eq!(result, Some("lib/my_app/bar.ex".to_string()));
    }

    #[test]
    fn elixir_resolver_returns_none_for_external() {
        let known = vec!["lib/my_app/bar.ex".to_string()];
        let result = ElixirParser.resolve("GenServer", "", &known);
        assert!(result.is_none());
    }

    /// Minimal inline `indoc`-style helper: strips common leading whitespace.
    fn indoc(s: &str) -> String {
        s.to_string()
    }
}
