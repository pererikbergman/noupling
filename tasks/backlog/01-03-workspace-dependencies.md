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

- [ ] clap (with derive feature) added for CLI parsing.
- [ ] tree-sitter and tree-sitter-rust added for AST parsing.
- [ ] rusqlite (with bundled feature) added for SQLite.
- [ ] rayon added for parallelism.
- [ ] fxhash added for efficient hashing.
- [ ] uuid (with v4 feature) added for ID generation.
- [ ] serde_json added for JSON output.
- [ ] `cargo build` compiles with all dependencies resolved.

### Implementation Steps

- [ ] 1. Add new dependencies to `[workspace.dependencies]` in root `Cargo.toml`.
- [ ] 2. Reference them in `crates/noupling/Cargo.toml`.
- [ ] 3. Verify `cargo build` succeeds.
