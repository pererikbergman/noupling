use tree_sitter::Parser;

pub struct ImportEntry {
    pub path: String,
    pub line_number: i32,
}

pub fn parse_rust_imports(source: &str) -> Vec<ImportEntry> {
    let mut parser = Parser::new();
    parser
        .set_language(&tree_sitter_rust::LANGUAGE.into())
        .expect("Failed to set Rust language");

    let tree = match parser.parse(source, None) {
        Some(t) => t,
        None => return Vec::new(),
    };

    let mut imports = Vec::new();
    collect_use_declarations(tree.root_node(), source, &mut imports);
    imports
}

fn collect_use_declarations(
    node: tree_sitter::Node,
    source: &str,
    imports: &mut Vec<ImportEntry>,
) {
    if node.kind() == "use_declaration" {
        let line_number = (node.start_position().row + 1) as i32;
        extract_paths_from_use(node, source, line_number, imports);
        return;
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_use_declarations(child, source, imports);
    }
}

fn extract_paths_from_use(
    node: tree_sitter::Node,
    source: &str,
    line_number: i32,
    imports: &mut Vec<ImportEntry>,
) {
    // Walk children to find the use path
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "scoped_identifier" | "identifier" | "scoped_use_list" | "use_wildcard" => {
                collect_paths_from_node(child, source, line_number, imports);
            }
            _ => {}
        }
    }
}

fn collect_paths_from_node(
    node: tree_sitter::Node,
    source: &str,
    line_number: i32,
    imports: &mut Vec<ImportEntry>,
) {
    match node.kind() {
        "scoped_use_list" => {
            // e.g., std::collections::{HashMap, HashSet}
            // Get the prefix path
            let mut prefix = String::new();
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                match child.kind() {
                    "scoped_identifier" | "identifier" => {
                        prefix = node_text(child, source);
                    }
                    "use_list" => {
                        let mut list_cursor = child.walk();
                        for list_child in child.children(&mut list_cursor) {
                            match list_child.kind() {
                                "identifier" => {
                                    let name = node_text(list_child, source);
                                    let full_path = if prefix.is_empty() {
                                        name
                                    } else {
                                        format!("{}::{}", prefix, name)
                                    };
                                    imports.push(ImportEntry {
                                        path: full_path,
                                        line_number,
                                    });
                                }
                                "scoped_identifier" => {
                                    let name = node_text(list_child, source);
                                    let full_path = if prefix.is_empty() {
                                        name
                                    } else {
                                        format!("{}::{}", prefix, name)
                                    };
                                    imports.push(ImportEntry {
                                        path: full_path,
                                        line_number,
                                    });
                                }
                                "self" => {
                                    imports.push(ImportEntry {
                                        path: prefix.clone(),
                                        line_number,
                                    });
                                }
                                _ => {}
                            }
                        }
                        return;
                    }
                    "::" => {}
                    _ => {}
                }
            }
            // If no use_list found, treat the whole thing as a single path
            let text = node_text(node, source);
            imports.push(ImportEntry {
                path: text,
                line_number,
            });
        }
        "scoped_identifier" | "identifier" => {
            let text = node_text(node, source);
            imports.push(ImportEntry {
                path: text,
                line_number,
            });
        }
        "use_wildcard" => {
            let text = node_text(node, source);
            imports.push(ImportEntry {
                path: text,
                line_number,
            });
        }
        _ => {}
    }
}

pub fn parse_kotlin_imports(source: &str) -> Vec<ImportEntry> {
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

fn collect_kotlin_imports(
    node: tree_sitter::Node,
    source: &str,
    imports: &mut Vec<ImportEntry>,
) {
    // tree-sitter-kotlin-ng uses "import" node with "qualified_identifier" child
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
        let path = full
            .trim_start_matches("import ")
            .trim()
            .to_string();
        if !path.is_empty() {
            imports.push(ImportEntry {
                path,
                line_number,
            });
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_simple_use() {
        let source = "use std::collections::HashMap;";
        let imports = parse_rust_imports(source);
        assert_eq!(imports.len(), 1);
        assert_eq!(imports[0].path, "std::collections::HashMap");
        assert_eq!(imports[0].line_number, 1);
    }

    #[test]
    fn parses_use_with_line_number() {
        let source = "\nuse std::io;\n";
        let imports = parse_rust_imports(source);
        assert_eq!(imports.len(), 1);
        assert_eq!(imports[0].path, "std::io");
        assert_eq!(imports[0].line_number, 2);
    }

    #[test]
    fn parses_crate_use() {
        let source = "use crate::core::Node;";
        let imports = parse_rust_imports(source);
        assert_eq!(imports.len(), 1);
        assert_eq!(imports[0].path, "crate::core::Node");
    }

    #[test]
    fn parses_use_list() {
        let source = "use std::collections::{HashMap, HashSet};";
        let imports = parse_rust_imports(source);
        assert_eq!(imports.len(), 2);
        let paths: Vec<&str> = imports.iter().map(|i| i.path.as_str()).collect();
        assert!(paths.contains(&"std::collections::HashMap"));
        assert!(paths.contains(&"std::collections::HashSet"));
    }

    #[test]
    fn parses_glob_use() {
        let source = "use std::io::*;";
        let imports = parse_rust_imports(source);
        assert_eq!(imports.len(), 1);
        assert_eq!(imports[0].path, "std::io::*");
    }

    #[test]
    fn parses_multiple_use_statements() {
        let source = "use std::io;\nuse std::fs;\nuse crate::utils;\n";
        let imports = parse_rust_imports(source);
        assert_eq!(imports.len(), 3);
        assert_eq!(imports[0].line_number, 1);
        assert_eq!(imports[1].line_number, 2);
        assert_eq!(imports[2].line_number, 3);
    }

    #[test]
    fn handles_empty_source() {
        let imports = parse_rust_imports("");
        assert!(imports.is_empty());
    }

    #[test]
    fn ignores_non_use_code() {
        let source = "fn main() { let x = 42; }";
        let imports = parse_rust_imports(source);
        assert!(imports.is_empty());
    }

    // ── Kotlin parser ──

    #[test]
    fn kotlin_parses_simple_import() {
        let source = "import com.example.MyClass";
        let imports = parse_kotlin_imports(source);
        assert_eq!(imports.len(), 1);
        assert_eq!(imports[0].path, "com.example.MyClass");
        assert_eq!(imports[0].line_number, 1);
    }

    #[test]
    fn kotlin_parses_multiple_imports() {
        let source = "import com.example.Foo\nimport com.example.Bar\nimport org.utils.Helper\n";
        let imports = parse_kotlin_imports(source);
        assert_eq!(imports.len(), 3);
        assert_eq!(imports[0].path, "com.example.Foo");
        assert_eq!(imports[1].path, "com.example.Bar");
        assert_eq!(imports[2].path, "org.utils.Helper");
    }

    #[test]
    fn kotlin_parses_wildcard_import() {
        let source = "import com.example.*";
        let imports = parse_kotlin_imports(source);
        assert_eq!(imports.len(), 1);
        assert!(imports[0].path.contains("com.example"));
    }

    #[test]
    fn kotlin_line_numbers_correct() {
        let source = "package com.example\n\nimport com.example.Foo\nimport com.example.Bar\n";
        let imports = parse_kotlin_imports(source);
        assert_eq!(imports.len(), 2);
        assert_eq!(imports[0].line_number, 3);
        assert_eq!(imports[1].line_number, 4);
    }

    #[test]
    fn kotlin_handles_empty_source() {
        let imports = parse_kotlin_imports("");
        assert!(imports.is_empty());
    }

    #[test]
    fn kotlin_ignores_non_import_code() {
        let source = "fun main() { println(\"hello\") }";
        let imports = parse_kotlin_imports(source);
        assert!(imports.is_empty());
    }
}
