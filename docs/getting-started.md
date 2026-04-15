# Getting Started

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

## Quick Start

### 1. Scan your project

```bash
noupling scan /path/to/project
```

This discovers source files, parses imports using Tree-sitter, and stores the dependency graph in `.noupling/history.db`. A `settings.json` is auto-created with sensible defaults on first run.

### 2. Check the health score

```bash
noupling audit /path/to/project
```

Output:

```
Health Score: 82.3/100
Total Modules: 47
Violations: 12
Total XS: 28 imports to remove

  [0.50] src/scanner/mod.rs -> src/core/mod.rs (depth 1)
         src/scanner <> src/core
  [0.33] CIRCULAR src/main -> src/analytics (depth 2)
         src/main <> src/analytics
         Weakest link: src/analytics -> src/main (3 imports)
```

### 3. Generate an HTML report

```bash
noupling report /path/to/project --format html
```

Open `.noupling/report/index.html` in your browser for an interactive drill-down view with color-coded scores per directory.

### 4. Set up CI gating

```bash
# In your CI pipeline:
noupling scan . --diff-base origin/main
noupling audit . --fail-below 80
```

This scans the full project, filters to changed files, and exits with code 1 if the score drops below 80.

## What noupling detects

### Coupling violations

Sibling directories that depend on each other. If `src/scanner/` imports from `src/core/`, that's a coupling violation at depth 1.

Severity: `weight / (depth + 1)`. Root-level coupling is most severe.

### Circular dependencies

Dependency chains that form loops: A imports B, B imports C, C imports A.

Severity: `modules / (depth + 1) / 10`. Always significant, amplified near the root.

### Health score

```
Score = 100 * (1 - sum_of_severities / total_modules)
```

A score of 100 means no violations. Below 70 is a warning. Below 50 needs attention.
