use std::path::Path;
use tree_sitter::Parser;

use super::{ImportEntry, LanguageParser};

pub struct PhpParser;

impl LanguageParser for PhpParser {
    fn parse(&self, source: &str) -> Vec<ImportEntry> {
        let mut parser = Parser::new();
        parser
            .set_language(&tree_sitter_php::LANGUAGE_PHP.into())
            .expect("Failed to set PHP language");

        let tree = match parser.parse(source, None) {
            Some(t) => t,
            None => return Vec::new(),
        };

        let mut imports = Vec::new();
        collect_php_imports(tree.root_node(), source, &mut imports);
        imports
    }

    fn resolve(
        &self,
        import_path: &str,
        source_file: &str,
        known_paths: &[String],
    ) -> Option<String> {
        resolve_php_import(import_path, source_file, known_paths)
    }
}

fn collect_php_imports(node: tree_sitter::Node, source: &str, imports: &mut Vec<ImportEntry>) {
    match node.kind() {
        "namespace_use_declaration" => {
            let line_number = (node.start_position().row + 1) as i32;
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                if child.kind() == "namespace_use_clause" {
                    let text = node_text(child, source).trim().replace('\\', "/");
                    if !text.is_empty() {
                        imports.push(ImportEntry {
                            path: text,
                            line_number,
                        });
                    }
                } else if child.kind() == "namespace_use_group" {
                    let mut group_cursor = child.walk();
                    for group_child in child.children(&mut group_cursor) {
                        if group_child.kind() == "namespace_use_clause" {
                            let text = node_text(group_child, source).trim().replace('\\', "/");
                            if !text.is_empty() {
                                imports.push(ImportEntry {
                                    path: text,
                                    line_number,
                                });
                            }
                        }
                    }
                }
            }
        }
        "include_expression"
        | "include_once_expression"
        | "require_expression"
        | "require_once_expression" => {
            let line_number = (node.start_position().row + 1) as i32;
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                if child.kind() == "string" || child.kind() == "encapsed_string" {
                    let text = node_text(child, source);
                    let path = text.trim_matches('\'').trim_matches('"').to_string();
                    if !path.is_empty() {
                        imports.push(ImportEntry { path, line_number });
                    }
                }
            }
        }
        _ => {
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                collect_php_imports(child, source, imports);
            }
        }
    }
}

fn node_text(node: tree_sitter::Node, source: &str) -> String {
    source[node.byte_range()].to_string()
}

// ── Resolver ──────────────────────────────────────────────────────────────────

fn normalize_path(path: &str) -> String {
    path.replace('\\', "/")
}

fn resolve_php_import(
    import_path: &str,
    source_file: &str,
    known_paths: &[String],
) -> Option<String> {
    let as_path = if !import_path.ends_with(".php") && import_path.contains('/') {
        format!("{}.php", import_path)
    } else {
        import_path.to_string()
    };

    if let Some(found) = known_paths.iter().find(|p| p.ends_with(&as_path)) {
        return Some(found.clone());
    }

    let source_dir = Path::new(source_file).parent()?;
    let resolved = normalize_path(&source_dir.join(import_path).to_string_lossy());
    known_paths
        .iter()
        .find(|p| **p == resolved || p.ends_with(&resolved))
        .cloned()
}

// ── Tests ──────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn php_parses_use_statement() {
        let source = "<?php\nuse App\\Models\\User;\nuse App\\Services\\AuthService;\n";
        let imports = PhpParser.parse(source);
        assert!(imports.len() >= 2, "got {} imports", imports.len());
    }

    #[test]
    fn php_parses_require() {
        let source = "<?php\nrequire_once 'vendor/autoload.php';\nrequire 'config.php';\n";
        let imports = PhpParser.parse(source);
        assert!(imports.len() >= 2, "got {} imports", imports.len());
    }

    #[test]
    fn php_handles_empty_source() {
        let imports = PhpParser.parse("");
        assert!(imports.is_empty());
    }
}
