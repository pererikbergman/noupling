# CLI Reference

## `noupling init [PATH]`

Create `.noupling/settings.json` with default configuration.

Settings are auto-created on first run of any command, but `init` lets you customize before scanning.

```bash
noupling init .
noupling init /path/to/project
```

## `noupling scan <PATH> [OPTIONS]`

Discover source files, parse imports via Tree-sitter, resolve dependencies, and store results in `.noupling/history.db`. Each scan creates a new snapshot with a unique ID.

```bash
noupling scan .
noupling scan /path/to/project
noupling scan . --diff-base main
noupling scan . --diff-base origin/develop
```

### Options

| Flag | Description |
|:---|:---|
| `--diff-base <BRANCH>` | Only report violations involving files changed compared to this branch. Scans the full project for dependency resolution but filters results to changed files only. |

## `noupling audit [PATH] [OPTIONS]`

Run coupling and circular dependency analysis on the latest (or specified) snapshot. Displays health score, violations, hotspots, and cohesion metrics to stdout.

```bash
noupling audit .
noupling audit /path/to/project
noupling audit . --fail-below 80
noupling audit . --baseline
noupling audit . --module app
```

### Options

| Flag | Description |
|:---|:---|
| `--snapshot <ID>` | Audit a specific snapshot instead of the latest one. |
| `--fail-below <SCORE>` | Exit with code 1 if health score is below this threshold. Use in CI. |
| `--baseline` | Compare against saved baseline. Only report NEW violations not in `.noupling/baseline.json`. |
| `--module <NAME>` | Show results for a specific module only (monorepo mode). |

## `noupling baseline <ACTION> [PATH]`

Save or manage the violation baseline for incremental adoption.

```bash
noupling baseline save .
```

Save current violations as accepted. Future audits with `--baseline` will only flag new violations.

## `noupling trend [PATH] [OPTIONS]`

Show health score history across snapshots to track architectural drift.

```bash
noupling trend .
noupling trend . --last 10
```

### Options

| Flag | Description |
|:---|:---|
| `--last <N>` | Show only the last N snapshots (default: 20). |

## `noupling report [PATH] [OPTIONS]`

Generate a report file from the latest snapshot's audit results.

```bash
noupling report . --format json
noupling report . --format xml
noupling report . --format md
noupling report . --format html
noupling report . --format sonar
noupling report . --format mermaid
noupling report . --format dot
```

### Options

| Flag | Description |
|:---|:---|
| `--format <FORMAT>` | Output format (required). See [Report Formats](report-formats.md). |
| `--module <NAME>` | Generate report for a specific module only (monorepo mode). |

## `noupling hook <ACTION> [PATH]`

Install or uninstall a git pre-commit hook.

```bash
noupling hook install .
noupling hook uninstall .
```

The hook scans changed files before each commit and fails if new violations are introduced. Bypass with `git commit --no-verify`.
