# Contributing to noupling

Thank you for your interest in contributing! This guide will help you get started.

## Getting Started

### Prerequisites

- Rust stable toolchain (`rustup default stable`)
- `rustfmt` and `clippy` components (`rustup component add rustfmt clippy`)

### Build and Test

```bash
git clone https://github.com/pererikbergman/noupling.git
cd noupling
cargo build
cargo test
```

### Run Against a Project

```bash
cargo run -- scan /path/to/project
cargo run -- audit /path/to/project
cargo run -- report /path/to/project --format html
```

## Coding Standards

- **Format**: Run `cargo fmt` before committing.
- **Lint**: Run `cargo clippy` and fix all warnings.
- **TDD**: Write tests first when adding new functionality.
- **Commits**: Use conventional commit messages (`feat:`, `fix:`, `docs:`, `refactor:`, `chore:`).

## Architecture

The project uses a flat module structure inside `src/`:

```
src/
├── main.rs          - CLI entry point
├── cli.rs           - Clap argument parsing
├── settings.rs      - Settings from .noupling/settings.json
├── diff.rs          - Git diff integration for PR/CI mode
├── core/            - Shared domain types (Module, Dependency, Snapshot)
├── scanner/         - File discovery, Tree-sitter parsing, import resolution
├── storage/         - SQLite persistence and repository patterns
├── analyzer/        - D_acc aggregation, BFS coupling audit, cycle detection
└── reporter/        - JSON, XML, Markdown, HTML, and SonarCloud report generation
```

## Adding a New Language Parser

1. Add the `tree-sitter-<lang>` dependency to `Cargo.toml`.
2. Add the file extension to `SOURCE_EXTENSIONS` in `settings.rs` (default list).
3. In `src/scanner/parser.rs`:
   - Add a `parse_<lang>_imports()` function using tree-sitter.
   - Add tests for the parser.
4. In `src/scanner/resolver.rs`:
   - Add a `resolve_<lang>_import()` function.
   - Add the extension to the `match` in `resolve_import()`.
   - Add resolver tests.
5. In `src/scanner/mod.rs`:
   - Add the extension to the `match` in `scan_project()`.
6. Run `cargo test` and `cargo clippy`.

## Submitting a Pull Request

1. Fork the repository and create a feature branch.
2. Make your changes following the coding standards above.
3. Ensure all tests pass: `cargo test`
4. Ensure no clippy warnings: `cargo clippy`
5. Ensure code is formatted: `cargo fmt --check`
6. Open a PR with a clear description of what changed and why.

## Reporting Issues

- Use the GitHub issue templates for bug reports and feature requests.
- Include the output of `noupling --version` and the command you ran.
- For scan issues, include the language and a minimal reproducible example.
