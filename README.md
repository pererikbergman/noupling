# noupling

A high-performance CLI tool that audits software architecture by quantifying coupling and cohesion through hierarchical dependency analysis.

## Supported Languages

- C# (`.cs`)
- Go (`.go`)
- Haskell (`.hs`)
- Java (`.java`)
- JavaScript (`.js`, `.jsx`)
- Kotlin (`.kt`, `.kts`)
- Python (`.py`)
- Rust (`.rs`)
- Swift (`.swift`)
- TypeScript (`.ts`, `.tsx`)
- Zig (`.zig`)

## Installation

```bash
cargo install --path crates/noupling
```

## Usage

### Scan a project

```bash
noupling scan /path/to/project
```

Discovers source files, parses imports via Tree-sitter, and stores the dependency graph in `.noupling/history.db`.

### Audit for coupling violations

```bash
noupling audit /path/to/project
```

Runs bottom-up dependency aggregation and top-down BFS coupling detection. Displays a health score (0-100) and any violations sorted by severity.

Use `--snapshot <ID>` to audit a specific historical snapshot instead of the latest.

### Generate reports

```bash
noupling report /path/to/project --format json
noupling report /path/to/project --format md
```

Outputs a structured report to stdout and saves it to `.noupling/report.json` or `.noupling/report.md`.

## Development

```bash
cargo build                # Build
cargo test                 # Run tests
cargo run -- scan .        # Run against this project
cargo clippy               # Lint
cargo fmt                  # Format
```

## Structure

- `crates/noupling/` - Main binary with vertical slice architecture
  - `slices/scanner/` - File discovery and Tree-sitter AST parsing
  - `slices/storage/` - SQLite persistence and repository patterns
  - `slices/analyzer/` - D_acc aggregation and BFS coupling audit
  - `slices/reporter/` - JSON and Markdown report generation
- `deploy/` - Dockerfile and docker-compose
- `docs/` - Specifications, epics, and roadmap
- `examples/` - Sample projects for testing (Kotlin)
- `tests/` - Integration test fixtures
