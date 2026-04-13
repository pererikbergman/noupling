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

### Initialize settings

```bash
noupling init /path/to/project
```

Creates `.noupling/settings.json` with default thresholds, ignore patterns, and source extensions. Settings are auto-created on first scan if missing.

### Scan a project

```bash
noupling scan /path/to/project
```

Discovers source files, parses imports via Tree-sitter, and stores the dependency graph in `.noupling/history.db`.

### Diff mode (PR/CI gate)

```bash
noupling scan /path/to/project --diff-base main
```

Scans the full project but only reports violations involving files changed compared to the base branch. Use this in CI pipelines to fail PRs only on new issues.

### Audit for coupling violations

```bash
noupling audit /path/to/project
```

Runs bottom-up dependency aggregation and top-down BFS coupling detection. Displays a health score (0-100) and any violations sorted by severity.

Use `--snapshot <ID>` to audit a specific historical snapshot instead of the latest.

### Generate reports

```bash
noupling report /path/to/project --format json    # Comprehensive JSON
noupling report /path/to/project --format xml     # Comprehensive XML
noupling report /path/to/project --format md      # Multi-file navigable Markdown
noupling report /path/to/project --format html    # Interactive HTML with drill-down
noupling report /path/to/project --format sonar   # SonarCloud generic issue import
```

Reports are saved to `.noupling/`. For SonarCloud, add to `sonar-project.properties`:

```
sonar.externalIssuesReportPaths=.noupling/noupling-sonar.json
```

## Configuration

Settings are stored in `.noupling/settings.json`:

```json
{
  "thresholds": {
    "score_green": 90.0,
    "score_yellow": 70.0,
    "critical_severity": 0.5,
    "minimum_severity": 0.2
  },
  "ignore_patterns": [
    "**/.git/**",
    "**/build/**",
    "**/generated/**",
    "**/node_modules/**"
  ],
  "source_extensions": ["rs", "kt", "java", "ts", "py"]
}
```

- **score_green/yellow**: Thresholds for color coding in HTML reports
- **critical_severity**: Violations above this are flagged as critical
- **minimum_severity**: Hide violations below this (reduces noise from deep coupling)
- **ignore_patterns**: Glob patterns (gitignore-style) for directories to skip
- **source_extensions**: File types to include in the scan

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
  - `slices/scanner/` - File discovery and Tree-sitter AST parsing (11 languages)
  - `slices/storage/` - SQLite persistence and repository patterns
  - `slices/analyzer/` - D_acc aggregation, BFS coupling audit, cycle detection
  - `slices/reporter/` - JSON, XML, Markdown, HTML, and SonarCloud report generation
- `deploy/` - Dockerfile
- `docs/` - Specifications and roadmap
- `examples/` - Sample Kotlin projects for testing
- `tests/` - Integration test fixtures
