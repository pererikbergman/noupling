---
description: Registry of third-party dependencies and their rationale to prevent redundancy
type: Knowledge
---

# Tech Stack

## Core Dependencies

| Dependency | Version | Rationale |
| :--- | :--- | :--- |
| `clap` | 4.0 (with `derive`) | CLI argument parsing with derive macros |
| `tree-sitter` | 0.24 | Incremental AST parsing engine |
| `tree-sitter-rust` | 0.24 | Rust grammar for Tree-sitter (first language) |
| `rusqlite` | 0.32 (with `bundled`) | SQLite persistence for snapshots, nodes, dependencies |
| `rayon` | 1.10 | Parallel file scanning and AST parsing |
| `fxhash` | 0.2 | Fast hashing for D_acc set union operations |
| `uuid` | 1.0 (with `v4`) | UUID generation for snapshot and node IDs |
| `serde` | 1.0 (with `derive`) | Serialization/deserialization framework |
| `serde_json` | 1.0 | JSON output for reports |
| `thiserror` | 1.0 | Ergonomic custom error type derivation |
| `anyhow` | 1.0 | Flexible error handling for application-level code |
| `tracing` | 0.1 | Structured logging and diagnostics |

## Toolchain

- **Channel:** stable (managed via `rust-toolchain.toml`)
- **Components:** rustfmt, clippy

## Notes

- All workspace dependencies are centralized in the root `Cargo.toml` under `[workspace.dependencies]`.
- Individual crates reference workspace dependencies using `{ workspace = true }`.
- `tokio` is declared in workspace but not yet used by noupling (may be needed for async operations later).
