---
description: Add all required workspace dependencies for noupling CLI
type: Task
story: 01-03
---

# Task: Add Workspace Dependencies

### Context

The workspace currently has generic dependencies. noupling needs specific crates for CLI parsing, Tree-sitter, SQLite, parallelism, and hashing.

### Objective

Declare all required dependencies in the workspace root `Cargo.toml` and reference them in the noupling crate.

### Acceptance Criteria

- [x] clap (with derive feature) added for CLI parsing.
- [x] tree-sitter and tree-sitter-rust added for AST parsing.
- [x] rusqlite (with bundled feature) added for SQLite.
- [x] rayon added for parallelism.
- [x] fxhash added for efficient hashing.
- [x] uuid (with v4 feature) added for ID generation.
- [x] serde_json added for JSON output.
- [x] `cargo build` compiles with all dependencies resolved.

### Implementation Steps

- [x] 1. Add new dependencies to `[workspace.dependencies]` in root `Cargo.toml`.
- [x] 2. Reference them in `crates/noupling/Cargo.toml`.
- [x] 3. Verify `cargo build` succeeds.
