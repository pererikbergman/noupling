---
name: add-language
description: Step-by-step procedure for adding a new language parser
---

# Add Language Parser

1. Add `tree-sitter-<lang>` dependency to `Cargo.toml`.
2. Add file extension to `default_source_extensions()` in `src/settings.rs`.
3. In `src/scanner/parser.rs`:
   - Add `pub fn parse_<lang>_imports(source: &str) -> Vec<ImportEntry>`.
   - Use tree-sitter to walk the AST and extract import nodes.
   - Add at least 3 tests (simple import, multiple imports, empty source).
4. In `src/scanner/resolver.rs`:
   - Add `fn resolve_<lang>_import(...)` function.
   - Add the extension to the `match` in `resolve_import()`.
   - Add resolver tests.
5. In `src/scanner/mod.rs`:
   - Add the extension to the `match` in `scan_project()`.
6. Update `README.md` supported languages list.
7. Run `cargo test && cargo clippy -- -D warnings`.
