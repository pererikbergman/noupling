---
description: Registry of third-party dependencies and their rationale to prevent redundancy
type: Knowledge
---

# Tech Stack

## Core Dependencies

| Dependency | Version | Rationale |
| :--- | :--- | :--- |
| `serde` | 1.0 (with `derive`) | Standard serialization/deserialization framework for Rust |
| `tokio` | 1.0 (with `full`) | Async runtime for the API application |
| `thiserror` | 1.0 | Ergonomic custom error type derivation |
| `anyhow` | 1.0 | Flexible error handling for application-level code |
| `tracing` | 0.1 | Structured logging and diagnostics |

## Toolchain

- **Channel:** stable (managed via `rust-toolchain.toml`)
- **Components:** rustfmt, clippy

## Notes

- All workspace dependencies are centralized in the root `Cargo.toml` under `[workspace.dependencies]`.
- Individual crates reference workspace dependencies using `{ workspace = true }`.
