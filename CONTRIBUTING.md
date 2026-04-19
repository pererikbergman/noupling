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

## Branching Strategy

- **`main`** is the stable branch. All PRs target `main`.
- Create feature branches from `main`: `feature/<issue-number>-short-description`
- One branch per issue. Reference the issue in your PR.
- PRs are squash-merged to keep history clean.

```bash
git checkout main
git pull
git checkout -b feature/42-add-ruby-parser
# ... make changes ...
git push -u origin feature/42-add-ruby-parser
gh pr create
```

## Coding Standards

- **Format**: Run `cargo fmt` before committing.
- **Lint**: Run `cargo clippy -- -D warnings` and fix all warnings. CI enforces zero warnings.
- **TDD**: Write tests first when adding new functionality (Red-Green-Refactor).
- **Commits**: Use conventional commit messages:
  - `feat(scanner): add Ruby parser (#42)`
  - `fix(analyzer): correct severity at depth 0`
  - `docs: update architecture diagram`
  - `refactor(storage): simplify query builder`
  - `chore: bump tree-sitter to 0.26`
- **CI must pass**: Check, format, clippy, and tests on Linux/macOS/Windows.

## Architecture

See [docs/architecture.md](docs/architecture.md) for the full data flow and module responsibilities.

```
src/
├── main.rs          - CLI entry point
├── cli.rs           - Clap argument parsing
├── settings.rs      - Settings from .noupling/settings.json
├── diff.rs          - Git diff integration for PR/CI mode
├── baseline.rs      - Baseline file management for incremental adoption
├── core/            - Shared domain types (Module, Dependency, Snapshot)
├── scanner/         - File discovery, Tree-sitter parsing, import resolution, external import counting
├── storage/         - SQLite persistence and repository patterns
├── analyzer/        - D_acc aggregation, BFS coupling audit, cycle detection, DependencyDirection classification, RRI/TRI computation, Gravity Well detection, Red Flag detection
└── reporter/        - JSON, XML, Markdown, HTML, SonarCloud, Mermaid, DOT, Bundle, Dashboard, PR, Briefing, Strategy report generation
```

## Adding a New Language Parser

This is the most common contribution. Follow these steps:

1. **Add dependency** to `Cargo.toml`:
   ```toml
   tree-sitter-ruby = "0.23"
   ```

2. **Add extension** to `default_source_extensions()` in `src/settings.rs`:
   ```rust
   "rb".to_string(),
   ```

3. **Add parser** in `src/scanner/parser.rs`:
   ```rust
   pub fn parse_ruby_imports(source: &str) -> Vec<ImportEntry> {
       // Use tree-sitter to parse imports
   }
   ```
   Add at least 3 tests: simple import, multiple imports, empty source.

4. **Add resolver** in `src/scanner/resolver.rs`:
   ```rust
   fn resolve_ruby_import(import_path: &str, known_paths: &[String]) -> Option<String> {
       // Map import to project file path
   }
   ```
   Add the extension to the `match` in `resolve_import()`. Add resolver tests.

5. **Route in scanner** in `src/scanner/mod.rs`:
   ```rust
   "rb" => parser::parse_ruby_imports(&source),
   ```

6. **Update README.md** supported languages list.

7. **Verify**:
   ```bash
   cargo test
   cargo clippy -- -D warnings
   cargo fmt --check
   ```

## Submitting a Pull Request

1. Fork the repository and create a feature branch from `main`.
2. Make your changes following the coding standards above.
3. Ensure all checks pass locally:
   ```bash
   cargo test
   cargo clippy -- -D warnings
   cargo fmt --check
   ```
4. Push and open a PR with a clear description.
5. Reference the GitHub Issue: `Closes #42`.
6. Wait for CI to pass on all platforms.

## Reporting Issues

- Use the [bug report](https://github.com/pererikbergman/noupling/issues/new?template=bug_report.md) or [feature request](https://github.com/pererikbergman/noupling/issues/new?template=feature_request.md) templates.
- Include the output of `noupling --version` and the command you ran.
- For scan issues, include the language and a minimal reproducible example.
