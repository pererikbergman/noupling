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

pub fn parse_typescript_imports(source: &str) -> Vec<ImportEntry> {
    let mut parser = Parser::new();
    let ts_lang: tree_sitter::Language = tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into();
    parser
        .set_language(&ts_lang)
        .expect("Failed to set TypeScript language");

    let tree = match parser.parse(source, None) {
        Some(t) => t,
        None => return Vec::new(),
    };

    let mut imports = Vec::new();
    collect_typescript_imports(tree.root_node(), source, &mut imports);
    imports
}

pub fn parse_tsx_imports(source: &str) -> Vec<ImportEntry> {
    let mut parser = Parser::new();
    let tsx_lang: tree_sitter::Language = tree_sitter_typescript::LANGUAGE_TSX.into();
    parser
        .set_language(&tsx_lang)
        .expect("Failed to set TSX language");

    let tree = match parser.parse(source, None) {
        Some(t) => t,
        None => return Vec::new(),
    };

    let mut imports = Vec::new();
    collect_typescript_imports(tree.root_node(), source, &mut imports);
    imports
}

fn collect_typescript_imports(
    node: tree_sitter::Node,
    source: &str,
    imports: &mut Vec<ImportEntry>,
) {
    if node.kind() == "import_statement" {
        let line_number = (node.start_position().row + 1) as i32;
        // Find the string/string_fragment child which contains the module path
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "string" {
                // Strip quotes from the string
                let text = node_text(child, source);
                let path = text.trim_matches(|c| c == '"' || c == '\'').to_string();
                if !path.is_empty() {
                    imports.push(ImportEntry {
                        path,
                        line_number,
                    });
                }
                return;
            }
        }
        return;
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_typescript_imports(child, source, imports);
    }
}

pub fn parse_swift_imports(source: &str) -> Vec<ImportEntry> {
    let mut parser = Parser::new();
    let swift_lang: tree_sitter::Language = tree_sitter_swift::LANGUAGE.into();
    parser
        .set_language(&swift_lang)
        .expect("Failed to set Swift language");

    let tree = match parser.parse(source, None) {
        Some(t) => t,
        None => return Vec::new(),
    };

    let mut imports = Vec::new();
    collect_swift_imports(tree.root_node(), source, &mut imports);
    imports
}

fn collect_swift_imports(
    node: tree_sitter::Node,
    source: &str,
    imports: &mut Vec<ImportEntry>,
) {
    if node.kind() == "import_declaration" {
        let line_number = (node.start_position().row + 1) as i32;
        // Extract the module identifier after "import"
        let full = node_text(node, source);
        let path = full.trim_start_matches("import ").trim().to_string();
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
        collect_swift_imports(child, source, imports);
    }
}

pub fn parse_csharp_imports(source: &str) -> Vec<ImportEntry> {
    let mut parser = Parser::new();
    let csharp_lang: tree_sitter::Language = tree_sitter_c_sharp::LANGUAGE.into();
    parser
        .set_language(&csharp_lang)
        .expect("Failed to set C# language");

    let tree = match parser.parse(source, None) {
        Some(t) => t,
        None => return Vec::new(),
    };

    let mut imports = Vec::new();
    collect_csharp_imports(tree.root_node(), source, &mut imports);
    imports
}

fn collect_csharp_imports(
    node: tree_sitter::Node,
    source: &str,
    imports: &mut Vec<ImportEntry>,
) {
    if node.kind() == "using_directive" {
        let line_number = (node.start_position().row + 1) as i32;
        // Find the qualified name child
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "qualified_name" || child.kind() == "identifier" {
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
            .trim_start_matches("using ")
            .trim_end_matches(';')
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
        collect_csharp_imports(child, source, imports);
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

    // ── TypeScript parser ──

    #[test]
    fn ts_parses_simple_import() {
        let source = "import { Component } from './component';";
        let imports = parse_typescript_imports(source);
        assert_eq!(imports.len(), 1);
        assert_eq!(imports[0].path, "./component");
        assert_eq!(imports[0].line_number, 1);
    }

    #[test]
    fn ts_parses_default_import() {
        let source = "import React from 'react';";
        let imports = parse_typescript_imports(source);
        assert_eq!(imports.len(), 1);
        assert_eq!(imports[0].path, "react");
    }

    #[test]
    fn ts_parses_namespace_import() {
        let source = "import * as utils from '../utils';";
        let imports = parse_typescript_imports(source);
        assert_eq!(imports.len(), 1);
        assert_eq!(imports[0].path, "../utils");
    }

    #[test]
    fn ts_parses_multiple_imports() {
        let source = "import { Foo } from './foo';\nimport { Bar } from './bar';\nimport { Baz } from '../baz';\n";
        let imports = parse_typescript_imports(source);
        assert_eq!(imports.len(), 3);
        assert_eq!(imports[0].line_number, 1);
        assert_eq!(imports[1].line_number, 2);
        assert_eq!(imports[2].line_number, 3);
    }

    #[test]
    fn ts_parses_relative_path() {
        let source = "import { helper } from '../../shared/helper';";
        let imports = parse_typescript_imports(source);
        assert_eq!(imports.len(), 1);
        assert_eq!(imports[0].path, "../../shared/helper");
    }

    #[test]
    fn ts_handles_empty_source() {
        let imports = parse_typescript_imports("");
        assert!(imports.is_empty());
    }

    #[test]
    fn ts_ignores_non_import_code() {
        let source = "const x = 42;\nfunction hello() {}";
        let imports = parse_typescript_imports(source);
        assert!(imports.is_empty());
    }

    #[test]
    fn tsx_parses_imports() {
        let source = "import { useState } from 'react';\nimport { Button } from './Button';";
        let imports = parse_tsx_imports(source);
        assert_eq!(imports.len(), 2);
        assert_eq!(imports[0].path, "react");
        assert_eq!(imports[1].path, "./Button");
    }

    // ── Swift parser ──

    #[test]
    fn swift_parses_simple_import() {
        let source = "import Foundation";
        let imports = parse_swift_imports(source);
        assert_eq!(imports.len(), 1);
        assert_eq!(imports[0].path, "Foundation");
        assert_eq!(imports[0].line_number, 1);
    }

    #[test]
    fn swift_parses_multiple_imports() {
        let source = "import UIKit\nimport SwiftUI\nimport Combine\n";
        let imports = parse_swift_imports(source);
        assert_eq!(imports.len(), 3);
        assert_eq!(imports[0].path, "UIKit");
        assert_eq!(imports[1].path, "SwiftUI");
        assert_eq!(imports[2].path, "Combine");
    }

    #[test]
    fn swift_handles_empty_source() {
        let imports = parse_swift_imports("");
        assert!(imports.is_empty());
    }

    #[test]
    fn swift_ignores_non_import_code() {
        let source = "func hello() { print(\"hello\") }";
        let imports = parse_swift_imports(source);
        assert!(imports.is_empty());
    }

    // ── C# parser ──

    #[test]
    fn csharp_parses_simple_using() {
        let source = "using System;";
        let imports = parse_csharp_imports(source);
        assert_eq!(imports.len(), 1);
        assert_eq!(imports[0].path, "System");
        assert_eq!(imports[0].line_number, 1);
    }

    #[test]
    fn csharp_parses_qualified_using() {
        let source = "using System.Collections.Generic;";
        let imports = parse_csharp_imports(source);
        assert_eq!(imports.len(), 1);
        assert_eq!(imports[0].path, "System.Collections.Generic");
    }

    #[test]
    fn csharp_parses_multiple_usings() {
        let source = "using System;\nusing System.Linq;\nusing MyApp.Data;\n";
        let imports = parse_csharp_imports(source);
        assert_eq!(imports.len(), 3);
        assert_eq!(imports[0].path, "System");
        assert_eq!(imports[1].path, "System.Linq");
        assert_eq!(imports[2].path, "MyApp.Data");
    }

    #[test]
    fn csharp_handles_empty_source() {
        let imports = parse_csharp_imports("");
        assert!(imports.is_empty());
    }

    #[test]
    fn csharp_ignores_non_using_code() {
        let source = "namespace MyApp { class Foo {} }";
        let imports = parse_csharp_imports(source);
        assert!(imports.is_empty());
    }
}
