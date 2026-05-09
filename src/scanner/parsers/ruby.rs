use std::path::Path;
use tree_sitter::Parser;

use super::{ImportEntry, LanguageParser};

pub struct RubyParser;

impl LanguageParser for RubyParser {
    fn parse(&self, source: &str) -> Vec<ImportEntry> {
        let mut parser = Parser::new();
        parser
            .set_language(&tree_sitter_ruby::LANGUAGE.into())
            .expect("Failed to set Ruby language");

        let tree = match parser.parse(source, None) {
            Some(t) => t,
            None => return Vec::new(),
        };

        let mut imports = Vec::new();
        collect_ruby_imports(tree.root_node(), source, &mut imports);
        imports
    }

    fn resolve(
        &self,
        import_path: &str,
        source_file: &str,
        known_paths: &[String],
    ) -> Option<String> {
        resolve_ruby_import(import_path, source_file, known_paths)
    }
}

fn collect_ruby_imports(node: tree_sitter::Node, source: &str, imports: &mut Vec<ImportEntry>) {
    if node.kind() == "call" {
        let mut cursor = node.walk();
        let mut method_name = String::new();
        for child in node.children(&mut cursor) {
            if child.kind() == "identifier" && method_name.is_empty() {
                method_name = node_text(child, source);
            }
        }
        if method_name == "require" || method_name == "require_relative" || method_name == "load" {
            let line_number = (node.start_position().row + 1) as i32;
            let mut cursor2 = node.walk();
            for child in node.children(&mut cursor2) {
                if child.kind() == "argument_list" {
                    let mut arg_cursor = child.walk();
                    for arg in child.children(&mut arg_cursor) {
                        if arg.kind() == "string" {
                            let text = node_text(arg, source);
                            let path = text.trim_matches('\'').trim_matches('"').to_string();
                            if !path.is_empty() {
                                imports.push(ImportEntry { path, line_number });
                            }
                            return;
                        }
                    }
                }
                if child.kind() == "string" {
                    let text = node_text(child, source);
                    let path = text.trim_matches('\'').trim_matches('"').to_string();
                    if !path.is_empty() {
                        imports.push(ImportEntry { path, line_number });
                    }
                    return;
                }
            }
        }
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_ruby_imports(child, source, imports);
    }
}

fn node_text(node: tree_sitter::Node, source: &str) -> String {
    source[node.byte_range()].to_string()
}

// ── Resolver ──────────────────────────────────────────────────────────────────

fn normalize_path(path: &str) -> String {
    path.replace('\\', "/")
}

fn resolve_ruby_import(
    import_path: &str,
    source_file: &str,
    known_paths: &[String],
) -> Option<String> {
    let with_ext = if import_path.ends_with(".rb") {
        import_path.to_string()
    } else {
        format!("{}.rb", import_path)
    };

    let source_dir = Path::new(source_file).parent()?;
    let resolved = normalize_path(&source_dir.join(&with_ext).to_string_lossy());
    if let Some(found) = known_paths
        .iter()
        .find(|p| **p == resolved || p.ends_with(&resolved))
    {
        return Some(found.clone());
    }

    known_paths.iter().find(|p| p.ends_with(&with_ext)).cloned()
}

// ── Tests ──────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ruby_parses_require() {
        let source = "require 'json'\nrequire 'net/http'\n";
        let imports = RubyParser.parse(source);
        assert_eq!(imports.len(), 2);
        assert_eq!(imports[0].path, "json");
        assert_eq!(imports[1].path, "net/http");
    }

    #[test]
    fn ruby_parses_require_relative() {
        let source = "require_relative 'lib/utils'\nrequire_relative '../models/user'\n";
        let imports = RubyParser.parse(source);
        assert_eq!(imports.len(), 2);
    }

    #[test]
    fn ruby_handles_empty_source() {
        let imports = RubyParser.parse("");
        assert!(imports.is_empty());
    }
}
