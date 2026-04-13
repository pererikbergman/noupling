---
description: High-level overview of project purpose, domain, and architectural boundaries
type: Knowledge
---

# Project Map

## Purpose

**noupling** is a high-performance CLI tool that audits software architecture by quantifying coupling and cohesion through hierarchical dependency analysis.

## Domain

Architecture auditing: scans source code across 11 languages (C#, Go, Haskell, Java, JavaScript, Kotlin, Python, Rust, Swift, TypeScript, Zig), extracts import dependencies via Tree-sitter, and computes coupling violations using bottom-up aggregation and top-down BFS.

## Architectural Boundaries (Vertical Slices)

| Module | Type | Responsibility |
| :--- | :--- | :--- |
| `cli` | Module | Clap CLI argument parsing (scan, audit, report commands) |
| `core` | Module | Shared domain types: Module, ModuleType, Dependency, Snapshot |
| `slices/scanner` | Slice | File discovery (Rayon) and Tree-sitter AST parsing |
| `slices/storage` | Slice | SQLite persistence, schema migrations, repository patterns |
| `slices/analyzer` | Slice | Bottom-up D_acc aggregation, top-down BFS coupling audit |
| `slices/reporter` | Slice | JSON and Markdown report generation |
| `utils` | Module | Error handling and logging utilities |

## Key Directories

- `crates/noupling/`: Single binary crate (vertical slice architecture)
- `deploy/`: Dockerfile and docker-compose for containerization
- `docs/`: Design documents, specifications, epics, stories, roadmap
- `scripts/`: Development and automation scripts
- `tests/`: Integration tests and fixtures
- `tasks/`: Backlog and completed task files
- `.agent/`: Agent rules, skills, workflows, and knowledge base
