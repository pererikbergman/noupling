use std::path::Path;
use tree_sitter::Parser;

use super::{ImportEntry, LanguageParser};

pub struct ZigParser;

impl LanguageParser for ZigParser {
    fn parse(&self, source: &str) -> Vec<ImportEntry> {
        let mut parser = Parser::new();
        let zig_lang: tree_sitter::Language = tree_sitter_zig::LANGUAGE.into();
        parser
            .set_language(&zig_lang)
            .expect("Failed to set Zig language");

        let tree = match parser.parse(source, None) {
            Some(t) => t,
            None => return Vec::new(),
        };

        let mut imports = Vec::new();
        collect_zig_imports(tree.root_node(), source, &mut imports);
        imports
    }

    fn resolve(
        &self,
        import_path: &str,
        source_file: &str,
        known_paths: &[String],
    ) -> Option<String> {
        resolve_zig_import(import_path, source_file, known_paths)
    }
}

fn collect_zig_imports(node: tree_sitter::Node, source: &str, imports: &mut Vec<ImportEntry>) {
    if node.kind() == "builtin_function" {
        let mut cursor = node.walk();
        let mut is_import = false;
        for child in node.children(&mut cursor) {
            if child.kind() == "builtin_identifier" && node_text(child, source) == "@import" {
                is_import = true;
            }
            if is_import && child.kind() == "arguments" {
                let line_number = (node.start_position().row + 1) as i32;
                let mut arg_cursor = child.walk();
                for arg_child in child.children(&mut arg_cursor) {
                    if arg_child.kind() == "string" {
                        let mut str_cursor = arg_child.walk();
                        for str_child in arg_child.children(&mut str_cursor) {
                            if str_child.kind() == "string_content" {
                                let path = node_text(str_child, source);
                                if !path.is_empty() {
                                    imports.push(ImportEntry { path, line_number });
                                }
                                return;
                            }
                        }
                    }
                }
            }
        }
        return;
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_zig_imports(child, source, imports);
    }
}

fn node_text(node: tree_sitter::Node, source: &str) -> String {
    source[node.byte_range()].to_string()
}

// ── Resolver ──────────────────────────────────────────────────────────────────

fn resolve_zig_import(
    import_path: &str,
    source_file: &str,
    known_paths: &[String],
) -> Option<String> {
    if import_path == "std" || import_path == "builtin" {
        return None;
    }
    let source_dir = Path::new(source_file).parent()?;
    let resolved = source_dir.join(import_path);
    let mut components: Vec<String> = Vec::new();
    for comp in resolved.components() {
        match comp {
            std::path::Component::ParentDir => {
                components.pop();
            }
            std::path::Component::CurDir => {}
            std::path::Component::Normal(s) => {
                components.push(s.to_string_lossy().to_string())
            }
            _ => {}
        }
    }
    let candidate = components.join("/");
    if known_paths.contains(&candidate) {
        return Some(candidate);
    }
    None
}

// ── Tests ──────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zig_parses_import() {
        let source = "const std = @import(\"std\");";
        let imports = ZigParser.parse(source);
        assert_eq!(imports.len(), 1);
        assert_eq!(imports[0].path, "std");
    }

    #[test]
    fn zig_parses_file_import() {
        let source = "const utils = @import(\"utils.zig\");";
        let imports = ZigParser.parse(source);
        assert_eq!(imports.len(), 1);
        assert_eq!(imports[0].path, "utils.zig");
    }

    #[test]
    fn zig_handles_empty_source() {
        let imports = ZigParser.parse("");
        assert!(imports.is_empty());
    }
}
