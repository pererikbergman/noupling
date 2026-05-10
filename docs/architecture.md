# Architecture

## Overview

noupling is a single Rust binary that scans source code, extracts import dependencies, and analyzes architectural coupling. The data flows through four stages:

```
scan -> store -> analyze -> report
```

## Data Flow

```
1. SCAN          2. STORE           3. ANALYZE           4. REPORT
+-----------+    +-----------+     +--------------+     +-----------+
| scanner/  | -> | storage/  | --> | analyzer/    | --> | reporter/ |
|           |    |           |     |              |     |           |
| discover  |    | SQLite DB |     | D_acc        |     | JSON      |
| parse     |    | snapshots |     | BFS audit    |     | XML       |
| resolve   |    | modules   |     | risk weights |     | Markdown  |
|           |    | deps      |     | cycles       |     | HTML      |
+-----------+    +-----------+     | score        |     | Sonar     |
                                   +--------------+     +-----------+
```

## Module Responsibilities

### `src/core/`
Shared domain types used by all modules: `Module`, `ModuleType`, `Dependency`, `Snapshot`. These are the data structures that flow between stages.

### `src/scanner/`
**Stage 1: Scan.** Discovers source files, parses them with Tree-sitter, and resolves import paths.

- `discovery.rs` - Recursive file walker. Filters by `source_extensions` and `ignore_patterns` from settings. Produces `Module` structs with relative paths.
- `parsers/mod.rs` - Defines the `LanguageParser` trait and the `registry()` function. The trait has two pure methods: `parse(source) -> Vec<ImportEntry>` and `resolve(import_path, source_file, known_paths) -> Option<String>`. Adding a new language means dropping one adapter file into `src/scanner/parsers/` and adding one line to `registry()` — no other files change.
- `parsers/<lang>.rs` - One file per language (14 files: `csharp`, `dart`, `go`, `haskell`, `java`, `javascript`, `kotlin`, `php`, `python`, `ruby`, `rust`, `swift`, `typescript`, `zig`). Each implements `LanguageParser` for its file extension(s).
- `mod.rs` - Orchestrates: discover files, look up the adapter per extension from `registry()`, parse imports in parallel (Rayon), resolve to dependencies. Unresolved imports are counted as external (third-party) dependencies per module and returned in `ScanResult::external_imports`.

### `src/storage/`
**Stage 2: Store.** SQLite persistence in `.noupling/history.db`.

- `db.rs` - `Database` struct. Auto-creates schema on first access. Schema:
  - `snapshots` — one row per scan. Columns: `id`, `timestamp`, `root_path`, `suppressed_count`, `diff_base`, `diff_changed_files`.
  - `modules` — one row per discovered source file.
  - `dependencies` — one row per resolved import edge.
  - `snapshot_external_deps` — one row per module per snapshot for external (unresolved) import counts.
  - No JSON sidecar files. All scan metadata lives in SQLite.
- `repository.rs` - `SnapshotRepository`, `ModuleRepository`, `DependencyRepository`. Each provides CRUD operations with parameterized queries.

### `src/analyzer/`
**Stage 3: Analyze.** The core algorithm, split into focused concern files.

- `mod.rs` - Orchestrator. Declares `AuditResult` and re-exports the public API. The canonical entry point for command handlers is `audit_with_settings(modules, deps, &settings)`, which runs the full 5-step pipeline in fixed order: severity filtering → coupling-mode adjustment → risk-weight RRI computation → layer-weight reductions → layer filtering. Command handlers call this function once; the old manual 5-step sequence is no longer repeated by callers.
- `coupling.rs` - D_acc aggregation and BFS sibling coupling detection.
- `cycles.rs` - Circular dependency detection at sibling level via Tarjan's SCC algorithm.
- `direction.rs` - `DependencyDirection` enum (`Downward`, `Sibling`, `Upward`, `External`, `Transitive`, `Circular`) and its risk-weight semantics. Re-exported as `analyzer::DependencyDirection`.
- `metrics.rs` - Fan-in/fan-out hotspots and external dependency metrics.
- `cohesion.rs` - Per-directory cohesion metrics.
- `independence.rs` - Per-module independence scores (internal vs. external dependency ratio).
- `gravity_wells.rs` - Gravity Well detection: modules whose aggregate RRI exceeds 2× the median.
- `red_flags.rs` - Architectural anti-pattern detection: *Fused Sibling* and *Trapped Child*.
- `layers.rs` - Layer ordering rules and `LayerViolation`.
- `rules.rs` - Custom dependency rules from `settings.json` and `RuleViolation`.
- `violation_age.rs` - New/recent/chronic violation classification across snapshot history.
- `critical_path.rs` - Maximum dependency chain depth and the critical path.
- `actions.rs` - `TopAction` recommendations derived from the audit result.
- `monorepo.rs` - Monorepo: per-module auditing and cross-module violation detection.
- `tests.rs` - Integration-level tests for the analyzer.

**Core algorithm:**
- **D_acc (Accumulated Dependencies):** For each directory, computes the union of all external dependencies from its subtree. Internal dependencies (within the same directory) are excluded.
- **BFS Coupling Detection:** Walks the directory tree top-down. At each level, checks sibling pairs: if D_acc(A) references a module inside B, that's a coupling violation.
- **Dependency Direction Classification:** Each coupling violation is classified by `DependencyDirection`, which determines the risk weight applied during scoring.
- **Circular Dependency Detection:** At each BFS level, builds a graph between siblings using D_acc and finds all strongly connected components via Tarjan's SCC algorithm.
- **Severity (RRI):** Risk-Relative Impact. `RRI = direction_weight * density`, where `direction_weight` comes from the `DependencyDirection` classification and `density` captures coupling intensity.
- **Health Score:** `100 * (1 - TRI / (total_modules * max_weight))`, where `TRI` (Total Risk Index) is the sum of all RRIs across the project.
- **Gravity Wells:** Modules whose aggregate RRI exceeds 2x the median RRI. These are the modules that disproportionately concentrate coupling risk.
- **Red Flags:** Specific anti-patterns detected during analysis. *Fused Sibling* identifies high-density sibling pairs that are tightly co-dependent. *Trapped Child* identifies modules with upward dependencies on their parent or ancestors.

### `src/commands/`
**Thin command handlers.** Each file owns exactly one CLI command. `src/main.rs` is pure dispatch (72 LOC): it parses CLI arguments via Clap, auto-creates `settings.json` if absent, and delegates to the appropriate handler.

- `init.rs` - `init` command.
- `scan.rs` - `scan` command. Stores `suppressed_count`, `diff_base`, `diff_changed_files`, and `external_imports` in SQLite after scanning.
- `audit.rs` - `audit` command. Calls `audit_with_settings`, then layers on command-specific steps: rule violations, layer violations, violation age, diff filtering, and baseline comparison.
- `report.rs` - `report` command.
- `trend.rs` - `trend` command.
- `baseline.rs` - `baseline` command.
- `hook.rs` - `hook` command.
- `mod.rs` - Shared helper (`find_db`).

### `src/reporter/`
**Stage 4: Report.** Generates output in multiple formats. All formats include risk-weighted metrics: RRI per violation, TRI for the project, direction badges (Downward/Sibling/Upward/Circular), Gravity Wells, and Red Flags.

- `mod.rs` - JSON report (comprehensive with directory tree), XML, SonarCloud, and text format.
- `html.rs` - Multi-file static HTML with Kover-style drill-down navigation.
- `md.rs` - Multi-file Markdown with navigable README.md per directory.
- `bundle.rs` - Zoomable D3.js sunburst visualization.
- `dashboard.rs` - Single-page Technical Leader Dashboard.
- `graph.rs` - Mermaid and DOT graph output.
- `strategy.rs` - Strategy report format.

### `src/cli.rs`
Clap argument parsing. Commands: `init`, `scan`, `audit`, `trend`, `report`, `baseline`, `hook`.

### `src/settings.rs`
Loads `.noupling/settings.json` with thresholds, ignore patterns, source extensions, and `risk_weights` configuration (per-direction weight overrides for Downward, Sibling, Upward, External, Transitive, and Circular). Layers support per-layer threshold fields: `allow_sibling`, `max_sibling_density`, and `reduced_sibling_weight`. Falls back to defaults if missing.

### `src/diff.rs`
Git integration for PR/CI mode. Shells out to `git diff --name-only <base>...HEAD` to get changed files.

## Key Design Decisions

1. **Forward slashes everywhere.** All paths are normalized to `/` regardless of OS. This ensures SQLite data and reports are portable.

2. **Scan everything, filter violations.** In diff mode, the full project is scanned (needed for import resolution), but only violations involving changed files are reported.

3. **Circular detection at sibling level.** Cycles are detected per BFS level using D_acc, not on the raw file dependency graph. This shows cycles at the directory level where they're actionable.

4. **Settings auto-created.** If `.noupling/settings.json` doesn't exist, it's created with defaults on any command. This avoids a mandatory init step.

5. **Risk-weighted scoring over depth-based severity.** The original severity formula (`1/(depth+1)`) treated all couplings equally regardless of their architectural impact. The risk-weighted approach (RRI = direction_weight x density) assigns higher weights to more harmful dependency directions (e.g., Circular > Upward > Sibling > Downward), producing scores that better reflect actual architectural risk.

6. **One adapter file per language.** The `LanguageParser` trait decouples parse and resolve logic from the scanner orchestrator. Each language lives entirely in `src/scanner/parsers/<lang>.rs`; the `registry()` function in `parsers/mod.rs` is the only place that maps file extensions to adapters.

7. **`audit_with_settings` as the canonical seam.** The 5-step pipeline (severity filter → coupling mode → risk weights → layer weights → layer filter) runs in a fixed order inside `audit_with_settings`. Command handlers do not repeat this sequence; they call the function once and layer on command-specific steps afterward.

8. **All scan metadata in SQLite.** `suppressed_count`, `diff_base`, `diff_changed_files`, and per-module external dependency counts are stored as columns on `snapshots` and rows in `snapshot_external_deps`. No JSON sidecar files.
