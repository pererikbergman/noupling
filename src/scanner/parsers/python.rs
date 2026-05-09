use std::path::Path;
use tree_sitter::Parser;

use super::{ImportEntry, LanguageParser};

pub struct PythonParser;

impl LanguageParser for PythonParser {
    fn parse(&self, source: &str) -> Vec<ImportEntry> {
        let mut parser = Parser::new();
        let py_lang: tree_sitter::Language = tree_sitter_python::LANGUAGE.into();
        parser
            .set_language(&py_lang)
            .expect("Failed to set Python language");

        let tree = match parser.parse(source, None) {
            Some(t) => t,
            None => return Vec::new(),
        };

        let mut imports = Vec::new();
        collect_python_imports(tree.root_node(), source, &mut imports);
        imports
    }

    fn resolve(
        &self,
        import_path: &str,
        source_file: &str,
        known_paths: &[String],
    ) -> Option<String> {
        resolve_python_import(import_path, source_file, known_paths)
    }
}

fn collect_python_imports(node: tree_sitter::Node, source: &str, imports: &mut Vec<ImportEntry>) {
    match node.kind() {
        "import_statement" => {
            let line_number = (node.start_position().row + 1) as i32;
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                if child.kind() == "dotted_name" {
                    let text = node_text(child, source);
                    imports.push(ImportEntry {
                        path: text,
                        line_number,
                    });
                }
            }
        }
        "import_from_statement" => {
            let line_number = (node.start_position().row + 1) as i32;
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                if child.kind() == "dotted_name" {
                    let text = node_text(child, source);
                    imports.push(ImportEntry {
                        path: text,
                        line_number,
                    });
                    return;
                }
                if child.kind() == "relative_import" {
                    let text = node_text(child, source);
                    imports.push(ImportEntry {
                        path: text,
                        line_number,
                    });
                    return;
                }
            }
        }
        _ => {
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                collect_python_imports(child, source, imports);
            }
        }
    }
}

fn node_text(node: tree_sitter::Node, source: &str) -> String {
    source[node.byte_range()].to_string()
}

// ── Resolver ──────────────────────────────────────────────────────────────────

fn resolve_python_import(
    import_path: &str,
    source_file: &str,
    known_paths: &[String],
) -> Option<String> {
    if import_path.starts_with('.') {
        let source_dir = Path::new(source_file).parent()?;
        let dots = import_path.chars().take_while(|c| *c == '.').count();
        let mut base = source_dir.to_path_buf();
        for _ in 1..dots {
            base = base.parent()?.to_path_buf();
        }
        let remainder = &import_path[dots..];
        if remainder.is_empty() {
            let candidate = base.join("__init__.py").to_string_lossy().to_string();
            if known_paths.contains(&candidate) {
                return Some(candidate);
            }
            return None;
        }
        let file_path = remainder.replace('.', "/");
        let candidate = format!("{}/{}.py", base.display(), file_path);
        if known_paths.contains(&candidate) {
            return Some(candidate);
        }
        let candidate = format!("{}/{}/__init__.py", base.display(), file_path);
        if known_paths.contains(&candidate) {
            return Some(candidate);
        }
        return None;
    }

    let file_path = import_path.replace('.', "/");
    let candidate = format!("{}.py", file_path);
    if let Some(found) = known_paths.iter().find(|p| p.ends_with(&candidate)) {
        return Some(found.clone());
    }
    let candidate = format!("{}/__init__.py", file_path);
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
    fn python_parses_import() {
        let source = "import os";
        let imports = PythonParser.parse(source);
        assert_eq!(imports.len(), 1);
        assert_eq!(imports[0].path, "os");
    }

    #[test]
    fn python_parses_from_import() {
        let source = "from os.path import join";
        let imports = PythonParser.parse(source);
        assert_eq!(imports.len(), 1);
        assert_eq!(imports[0].path, "os.path");
    }

    #[test]
    fn python_parses_multiple_imports() {
        let source = "import os\nimport sys\nfrom pathlib import Path\n";
        let imports = PythonParser.parse(source);
        assert_eq!(imports.len(), 3);
    }

    #[test]
    fn python_handles_empty_source() {
        let imports = PythonParser.parse("");
        assert!(imports.is_empty());
    }
}
