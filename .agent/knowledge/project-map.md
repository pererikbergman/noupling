---
description: High-level overview of project purpose, domain, and architecture
---

# Project Map

## Purpose

**noupling** is a high-performance CLI tool that audits software architecture by detecting coupling violations and circular dependencies across 11 programming languages.

## Domain

Architecture auditing: scans source code, extracts import dependencies via Tree-sitter, and computes coupling violations using bottom-up D_acc aggregation and top-down BFS analysis.

## Architecture

Flat module structure in `src/`:

| Module | Responsibility |
| :--- | :--- |
| `cli.rs` | Clap CLI argument parsing (init, scan, audit, report) |
| `core/` | Shared domain types: Module, ModuleType, Dependency, Snapshot |
| `scanner/` | File discovery, Tree-sitter parsing (11 languages), import resolution |
| `storage/` | SQLite persistence (snapshots, modules, dependencies) |
| `analyzer/` | D_acc aggregation, BFS coupling audit, cycle detection |
| `reporter/` | JSON, XML, Markdown, HTML, SonarCloud report generation |
| `settings.rs` | Settings from .noupling/settings.json |
| `diff.rs` | Git diff integration for PR/CI mode |

## Project Management

- **Milestones**: GitHub Milestones (one per Epic)
- **Issues**: GitHub Issues linked to milestones
- **Board**: GitHub Project "noupling Roadmap"
- **Repository**: https://github.com/pererikbergman/noupling
