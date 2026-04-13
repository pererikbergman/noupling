---
description: File discovery and Tree-sitter AST parsing for dependency extraction
type: Spec
---

# Epic: Scanner Slice

### 1. Objective & Value Proposition

* **Goal:** Implement parallel file discovery and Tree-sitter based import parsing to build the dependency graph.
* **User Value:** Core capability: scans a project directory and extracts all import relationships.

### 2. User Stories

* As a user, I want to scan a project directory so that all source files are discovered recursively.
* As a user, I want Rust files parsed for `use` declarations so that dependencies are extracted.
* As a developer, I want file scanning to run in parallel (Rayon) so that large projects are handled efficiently.
* As a developer, I want a pluggable grammar system so that new languages can be added later.
* As a user, I want unsupported file types to be skipped with a warning so that the scan doesn't crash.

### 3. Functional Requirements

* Recursively discover files in target directory, building a tree of DIR and FILE nodes with depth tracking.
* Use Rayon for parallel file reading and AST parsing.
* Implement Tree-sitter Rust grammar: query `use_declaration` for `path_expression` imports.
* Map parsed imports to fully qualified file paths within the project.
* Skip files with unsupported grammars, logging a warning.

### 4. Acceptance Criteria

- [ ] File tree is built correctly with parent/child relationships and depth.
- [ ] Rust `use` statements are parsed and resolved to project file paths.
- [ ] Parallel scanning works via Rayon thread pool.
- [ ] Unsupported file types produce a warning, not an error.
- [ ] Integration test passes against a mock Rust project in `tests/fixtures/`.
