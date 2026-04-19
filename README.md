<p align="center">
  <h1 align="center">noupling</h1>
  <p align="center">
    <strong>Detect coupling violations and circular dependencies in your codebase.</strong>
  </p>
  <p align="center">
    <a href="https://github.com/pererikbergman/noupling/actions/workflows/ci.yml"><img src="https://github.com/pererikbergman/noupling/actions/workflows/ci.yml/badge.svg" alt="CI"></a>
    <a href="https://github.com/pererikbergman/noupling/blob/main/LICENSE"><img src="https://img.shields.io/badge/license-MIT-blue.svg" alt="License: MIT"></a>
    <img src="https://img.shields.io/badge/rust-2021-orange.svg" alt="Rust 2021">
    <img src="https://img.shields.io/badge/languages-14-green.svg" alt="14 Languages">
  </p>
</p>

---

## Why noupling?

Most linters check code style. **noupling checks architecture.**

It scans your project, builds a dependency graph from actual import statements, and quantifies how coupled your modules are using risk-weighted scoring (RRI/TRI). It finds:

- **Coupling violations** - sibling modules that depend on each other, breaking architectural boundaries
- **Circular dependencies** - dependency chains that form loops (A -> B -> C -> A), preventing independent development and testing

Every violation gets a **risk score (RRI)** based on dependency direction and density: upward and circular dependencies carry higher risk weights than downward ones. The result is a single **health score (0-100)** you can track over time and gate in CI.

### Key Features

- **14 languages**: C#, Dart, Go, Haskell, Java, JavaScript, Kotlin, PHP, Python, Ruby, Rust, Swift, TypeScript, Zig
- **Tree-sitter parsing**: Fast, accurate AST-based import extraction (no regex)
- **Parallel scanning**: Rayon-powered file discovery and parsing
- **12 report formats**: JSON, XML, Markdown, HTML, SonarCloud, Mermaid, DOT, Sunburst, Dashboard, PR, Briefing, Strategy (or `all` to generate every format)
- **Interactive HTML report**: Kover-style drill-down with color-coded scores
- **Sunburst visualization**: Zoomable D3.js dependency graph with animated drill-down
- **Technical Leader Dashboard**: Single-page executive view with all metrics, sortable scorecard, risk matrix
- **Monorepo support**: Independent analysis per module with cross-module dependency validation
- **Architectural layers**: Define dependency direction, suppress legitimate downward coupling
- **XS metric**: Quantify refactoring cost per violation, find the weakest link in cycles
- **Inline suppression**: `// noupling:ignore` to suppress known acceptable coupling
- **Advanced metrics**: Module independence, blast radius, instability (Martin's I), dependency depth, violation age
- **Per-module trends**: `--by-module` flag to track score evolution per directory across snapshots
- **PR/CI mode**: `--diff-base main` to only flag new violations
- **Risk-weighted scoring (RRI/TRI)**: Quantify coupling risk by dependency direction and density
- **Gravity Well detection**: Identify modules that attract excessive inbound dependencies
- **Red Flags (Fused Sibling, Trapped Child)**: Detect structural anti-patterns in module relationships
- **External dependency tracking**: Count third-party (unresolved) imports per module
- **Configurable risk weights per dependency direction**: Tune scoring to match your architecture's priorities
- **Configurable**: Thresholds, layers, dependency rules, glob ignore patterns

<p align="center">
  <img src="docs/noupling-html.png" alt="noupling HTML Report" width="800">
</p>

---

## Quick Start

```bash
# Install
cargo install --path .

# Scan your project
noupling scan /path/to/project

# See the health score
noupling audit /path/to/project

# Generate an interactive HTML report
noupling report /path/to/project --format html
```

---

## Installation

### Homebrew (macOS/Linux)

```bash
brew tap pererikbergman/noupling
brew install noupling
```

### From source

```bash
git clone https://github.com/pererikbergman/noupling.git
cd noupling
cargo install --path .
```

### Prebuilt binaries

Download from [GitHub Releases](https://github.com/pererikbergman/noupling/releases). Available for Linux (x86_64, aarch64), macOS (Apple Silicon, Intel), and Windows.

---

## Usage

### Scan a project

```bash
noupling scan /path/to/project
```

Discovers source files, parses imports via Tree-sitter, and stores the dependency graph in `.noupling/history.db`.

### Audit for violations

```bash
noupling audit /path/to/project
```

Displays a health score (0-100), coupling violations sorted by severity, and circular dependencies grouped by cycle order.

Use `--fail-below` to fail in CI when the score drops:

```bash
noupling audit /path/to/project --fail-below 80  # Exit code 1 if score < 80
```

### Generate reports

```bash
noupling report /path/to/project --format json      # Comprehensive JSON
noupling report /path/to/project --format xml       # Comprehensive XML
noupling report /path/to/project --format md        # Multi-file navigable Markdown
noupling report /path/to/project --format html      # Interactive HTML with drill-down
noupling report /path/to/project --format sonar     # SonarCloud generic issue import
noupling report /path/to/project --format mermaid   # Mermaid flowchart diagram
noupling report /path/to/project --format dot       # GraphViz DOT graph
noupling report /path/to/project --format bundle    # Zoomable sunburst with dependency edges
noupling report /path/to/project --format dashboard # Interactive Technical Leader Dashboard
noupling report /path/to/project --format all       # Generate every format above in one command
```

### Diff mode (PR/CI gate)

Only report violations from files changed compared to a base branch:

```bash
noupling scan /path/to/project --diff-base main
noupling audit /path/to/project
```

Scans the full project for import resolution but filters results to changed files only. Use this in CI to fail PRs only on **new** issues.

---

## CI/CD Integration

### GitHub Actions

```yaml
- name: Install noupling
  run: cargo install --path .

- name: Scan (diff mode)
  run: noupling scan . --diff-base origin/main

- name: Audit (fail if score drops below 80)
  run: noupling audit . --fail-below 80

- name: Generate Sonar report
  run: noupling report . --format sonar
```

### PR Comment Bot

Copy `.github/workflows/noupling-pr.yml` from this repo to your project. It automatically comments on PRs with:
- Health score
- Total and new violations
- Violation details

The workflow installs noupling from the latest release, runs a diff scan, and posts a comment. Updates existing comments on force-pushes.

### SonarCloud

Generate the generic issue import file and reference it in your Sonar config:

```bash
noupling report . --format sonar
```

Add to `sonar-project.properties`:

```properties
sonar.externalIssuesReportPaths=.noupling/noupling-sonar.json
```

---

## Configuration

Settings are stored in `.noupling/settings.json` (auto-created on first run):

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
  "source_extensions": [
    "rs", "kt", "java", "ts", "py", "swift", "cs",
    "go", "hs", "js", "jsx", "kts", "tsx", "zig",
    "dart", "php", "rb"
  ],
  "allow_inline_suppression": true,
  "risk_weights": {
    "downward": 2,
    "sibling": 4,
    "upward": 6,
    "external": 8,
    "transitive": 9,
    "circular": 10
  },
  "coupling_mode": "strict"
}
```

| Setting | Description | Default |
| :--- | :--- | :--- |
| `score_green` | Score threshold for "healthy" (green) | 90.0 |
| `score_yellow` | Score threshold for "warning" (yellow) | 70.0 |
| `critical_severity` | Violations above this are flagged critical | 0.5 |
| `minimum_severity` | Hide violations below this (reduce noise) | 0.2 |
| `ignore_patterns` | Glob patterns for dirs/files to skip | 15 defaults |
| `source_extensions` | File types to scan | 17 extensions |
| `allow_inline_suppression` | Enable `noupling:ignore` comments | true |
| `risk_weights` | Risk weights per dependency direction (downward, sibling, upward, external, transitive, circular) | 2, 4, 6, 8, 9, 10 |
| `coupling_mode` | Coupling analysis mode (`strict` or `relaxed`) | `strict` |

### Architectural Layers

Define layers to suppress coupling violations that follow the intended dependency direction:

```json
{
  "layers": [
    { "name": "presentation", "pattern": "**/ui/**", "allow_sibling": false, "max_sibling_density": 3, "reduced_sibling_weight": 2 },
    { "name": "domain", "pattern": "**/domain/**" },
    { "name": "data", "pattern": "**/data/**" }
  ]
}
```

Dependencies may only flow **downward** (presentation -> domain -> data). Downward dependencies are not flagged as coupling violations. Upward dependencies (data -> presentation) are flagged as both coupling and layer violations.

### Custom Dependency Rules

Forbid specific dependency patterns:

```json
{
  "dependency_rules": [
    { "from": "**/ui/**", "to": "**/data/**", "allow": false,
      "message": "UI must not depend on data layer directly" }
  ]
}
```

### Monorepo Modules

Define independent modules within a monorepo, each analyzed separately:

```json
{
  "modules": [
    { "name": "app", "path": "app/src", "depends_on": ["lib-core"] },
    { "name": "lib-core", "path": "lib/core/src", "depends_on": [] }
  ]
}
```

Cross-module imports not listed in `depends_on` are flagged as violations. Use `--module app` to audit or report a single module.

### Inline Suppression

Suppress known acceptable coupling with a comment on the import line:

```kotlin
import com.example.legacy.Helper // noupling:ignore
```

Works with `//`, `#`, and `--` comment styles. Disable with `"allow_inline_suppression": false`.

---

## How It Works

1. **Scan**: Discover source files, parse imports with Tree-sitter, resolve to project paths
2. **Store**: Persist modules and dependencies in SQLite (`.noupling/history.db`)
3. **Analyze**:
   - **D_acc**: For each directory, compute the union of all external dependencies from its subtree
   - **BFS**: Walk the tree top-down, checking sibling pairs for coupling
   - **Cycles**: Find circular dependencies among siblings at each level
4. **Score**: Each dependency is classified by direction and assigned a risk weight:
   - **Downward** (weight 2) - following the intended layer direction
   - **Sibling** (weight 4) - coupling between peer modules
   - **Upward** (weight 6) - violating the layer direction
   - **External** (weight 8) - dependency on a third-party (unresolved) import
   - **Transitive** (weight 9) - indirect dependency through intermediate modules
   - **Circular** (weight 10) - part of a dependency cycle

   **RRI** (Relationship Risk Index) = `direction_weight × density` (number of imports)

   **TRI** (Total Risk Index) = sum of all RRIs

   **Health Score** = `100 × (1 - TRI / (total_modules × max_weight))`

See [docs/architecture.md](docs/architecture.md) for the full technical details.

---

## Supported Languages

| Language | Extensions | Import Pattern |
| :--- | :--- | :--- |
| C# | `.cs` | `using` directives |
| Dart | `.dart` | `import` directives |
| Go | `.go` | `import` declarations |
| Haskell | `.hs` | `import` declarations |
| Java | `.java` | `import` declarations |
| JavaScript | `.js`, `.jsx` | ES `import` statements |
| Kotlin | `.kt`, `.kts` | `import` declarations |
| PHP | `.php` | `use` / `require` / `include` |
| Python | `.py` | `import` / `from...import` |
| Ruby | `.rb` | `require` / `require_relative` |
| Rust | `.rs` | `use` declarations |
| Swift | `.swift` | `import` declarations |
| TypeScript | `.ts`, `.tsx` | ES `import` statements |
| Zig | `.zig` | `@import()` builtins |

---

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for build instructions, coding standards, branching strategy, and how to add a new language parser.

## Security

If you discover a security vulnerability, please report it privately via [GitHub Security Advisories](https://github.com/pererikbergman/noupling/security/advisories).

## License

[MIT](LICENSE)
