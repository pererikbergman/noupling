---
description: Registry of third-party dependencies and their rationale to prevent redundancy
type: Knowledge
---

# Tech Stack

## Core Dependencies

| Dependency | Version | Rationale |
| :--- | :--- | :--- |
| `clap` | 4.0 (with `derive`) | CLI argument parsing with derive macros |
| `rusqlite` | 0.32 (with `bundled`) | SQLite persistence for snapshots, modules, dependencies |
| `rayon` | 1.10 | Parallel file scanning and AST parsing |
| `fxhash` | 0.2 | Fast hashing for D_acc set union operations |
| `uuid` | 1.0 (with `v4`) | UUID generation for snapshot and module IDs |
| `serde` | 1.0 (with `derive`) | Serialization/deserialization framework |
| `serde_json` | 1.0 | JSON output for reports |
| `thiserror` | 1.0 | Ergonomic custom error type derivation |
| `anyhow` | 1.0 | Flexible error handling for application-level code |
| `tracing` | 0.1 | Structured logging and diagnostics |

## Tree-sitter Grammars (11 languages)

| Dependency | Version | Language | Extensions |
| :--- | :--- | :--- | :--- |
| `tree-sitter` | 0.25 | Core parsing engine | - |
| `tree-sitter-rust` | 0.24 | Rust | `.rs` |
| `tree-sitter-kotlin-ng` | 1.1 | Kotlin | `.kt`, `.kts` |
| `tree-sitter-typescript` | 0.23 | TypeScript/TSX | `.ts`, `.tsx` |
| `tree-sitter-swift` | 0.7 | Swift | `.swift` |
| `tree-sitter-c-sharp` | 0.23 | C# | `.cs` |
| `tree-sitter-go` | 0.25 | Go | `.go` |
| `tree-sitter-haskell` | 0.23 | Haskell | `.hs` |
| `tree-sitter-java` | 0.23 | Java | `.java` |
| `tree-sitter-javascript` | 0.25 | JavaScript | `.js`, `.jsx` |
| `tree-sitter-python` | 0.25 | Python | `.py` |
| `tree-sitter-zig` | 1.1 | Zig | `.zig` |

## Toolchain

- **Channel:** stable (managed via `rust-toolchain.toml`)
- **Components:** rustfmt, clippy

## Notes

- All workspace dependencies are centralized in the root `Cargo.toml` under `[workspace.dependencies]`.
- Individual crates reference workspace dependencies using `{ workspace = true }`.
- `tokio` is declared in workspace but not used by noupling.
- `tree-sitter-kotlin-ng` is used instead of `tree-sitter-kotlin` for tree-sitter 0.25 ABI compatibility.
