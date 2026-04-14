# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.2.0] - 2026-04-14

### Added

- **Hotspot detection**: Fan-in/fan-out metrics per module. Identifies architectural bottlenecks (God modules).
- **Baseline file**: `noupling baseline save` and `noupling audit --baseline` for incremental adoption. Only fail on new violations.
- **Pre-commit hook**: `noupling hook install/uninstall` to block commits that introduce violations.
- **Exit code threshold**: `noupling audit --fail-below 80` for CI gating.
- **Homebrew tap**: `brew tap pererikbergman/noupling && brew install noupling`.

### Fixed

- Windows path separator normalization in scanner and resolver.
- SHA256 checksum generation on Windows in release workflow.

## [0.1.0] - 2026-04-14

### Added

- Initial release of noupling architecture auditing CLI.
- **11 language parsers**: C#, Go, Haskell, Java, JavaScript, Kotlin, Python, Rust, Swift, TypeScript, Zig.
- **Coupling detection**: Bottom-up D_acc aggregation with top-down BFS sibling analysis.
- **Circular dependency detection**: All cycle orders found per directory level using DFS.
- **5 report formats**: JSON, XML, multi-file Markdown, interactive HTML, SonarCloud.
- **Diff mode**: `--diff-base` flag for PR/CI gating (only report violations from changed files).
- **Configurable settings**: `.noupling/settings.json` with thresholds, glob ignore patterns, and source extensions.
- **Health score**: 0-100 score with depth-weighted severity for coupling and amplified severity for circular deps.
- **SQLite storage**: Snapshot-based persistence for trend analysis.
- **Interactive HTML report**: Kover-style drill-down navigation with color-coded scores.
