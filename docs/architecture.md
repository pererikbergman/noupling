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
Shared domain types used by all modules: `Module`, `ModuleType`, `Dependency`, `Snapshot`, `DependencyDirection`. The `DependencyDirection` enum classifies each dependency as Downward, Sibling, Upward, or Circular, which determines its risk weight during analysis. These are the data structures that flow between stages.

### `src/scanner/`
**Stage 1: Scan.** Discovers source files, parses them with Tree-sitter, and resolves import paths.

- `discovery.rs` - Recursive file walker. Filters by `source_extensions` and `ignore_patterns` from settings. Produces `Module` structs with relative paths.
- `parser.rs` - Tree-sitter parsers for 11 languages. Each `parse_<lang>_imports()` function extracts import statements with line numbers.
- `resolver.rs` - Maps parsed imports to actual file paths in the project. Language-specific resolution (Rust `crate::`, Kotlin dot-separated, TypeScript relative, etc.).
- `mod.rs` - Orchestrates: discover files, parse imports in parallel (Rayon), resolve to dependencies. Unresolved imports are counted as external (third-party) dependencies per module.

### `src/storage/`
**Stage 2: Store.** SQLite persistence in `.noupling/history.db`.

- `db.rs` - `Database` struct. Auto-creates schema (snapshots, modules, dependencies tables) on first access.
- `repository.rs` - `SnapshotRepository`, `ModuleRepository`, `DependencyRepository`. Each provides CRUD operations with parameterized queries.

### `src/analyzer/`
**Stage 3: Analyze.** The core algorithm.

- **D_acc (Accumulated Dependencies):** For each directory, computes the union of all external dependencies from its subtree. Internal dependencies (within the same directory) are excluded.
- **BFS Coupling Detection:** Walks the directory tree top-down. At each level, checks sibling pairs: if D_acc(A) references a module inside B, that's a coupling violation.
- **Dependency Direction Classification:** Each coupling violation is classified by `DependencyDirection` (Downward, Sibling, Upward, or Circular), which determines the risk weight applied during scoring.
- **Circular Dependency Detection:** At each BFS level, builds a graph between siblings using D_acc and finds all strongly connected components via Tarjan's SCC algorithm (replaced DFS-based cycle detection).
- **Severity (RRI):** Risk-Relative Impact. `RRI = direction_weight * density`, where `direction_weight` comes from the `DependencyDirection` classification and `density` captures coupling intensity.
- **Health Score:** `100 * (1 - TRI / (total_modules * max_weight))`, where `TRI` (Total Risk Index) is the sum of all RRIs across the project.
- **Gravity Wells:** Modules whose aggregate RRI exceeds 2x the median RRI. These are the modules that disproportionately concentrate coupling risk.
- **Red Flags:** Specific anti-patterns detected during analysis. *Fused Sibling* identifies high-density sibling pairs that are tightly co-dependent. *Trapped Child* identifies modules with upward dependencies on their parent or ancestors.

### `src/reporter/`
**Stage 4: Report.** Generates output in 6 formats. All formats now include risk-weighted metrics: RRI per violation, TRI for the project, direction badges (Downward/Sibling/Upward/Circular), Gravity Wells, and Red Flags.

- `mod.rs` - JSON report (comprehensive with directory tree), XML, SonarCloud, and text format.
- `html.rs` - Multi-file static HTML with Kover-style drill-down navigation.
- `md.rs` - Multi-file Markdown with navigable README.md per directory.

### `src/cli.rs`
Clap argument parsing. Commands: `init`, `scan`, `audit`, `report`.

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
