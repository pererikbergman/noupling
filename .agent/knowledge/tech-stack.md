---
description: Dependencies and their rationale to prevent redundancy
---

# Tech Stack

## Core Dependencies

| Dependency | Purpose |
| :--- | :--- |
| `clap` 4.0 | CLI argument parsing |
| `rusqlite` 0.32 (bundled) | SQLite persistence |
| `rayon` 1.10 | Parallel file scanning |
| `fxhash` 0.2 | Fast hashing for set operations |
| `globset` 0.4 | Gitignore-style pattern matching |
| `uuid` 1.0 | Snapshot/module ID generation |
| `serde` + `serde_json` 1.0 | Serialization |
| `thiserror` + `anyhow` 1.0 | Error handling |
| `tracing` 0.1 | Structured logging |

## Tree-sitter Grammars (11 languages)

tree-sitter 0.25 with: rust 0.24, kotlin-ng 1.1, typescript 0.23, swift 0.7, c-sharp 0.23, go 0.25, haskell 0.23, java 0.23, javascript 0.25, python 0.25, zig 1.1

## Toolchain

- Rust stable via `rust-toolchain.toml`
- Components: rustfmt, clippy
