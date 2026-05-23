# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- **Per-directory instability metric (Martin's I)** (#68): `I = Ce / (Ca + Ce)` per directory, derived from cross-directory dependency edges. Per-file instability already existed on `ModuleMetrics` (used by the dashboard and the text "Zone of Pain" section); the new aggregate operates at the package level, which is where Martin's metric is conventionally applied and where it composes with abstractness for the Distance from Main Sequence (#70). Internal (same-directory) edges are excluded. Surfaced as a new `Instability:` text section, an `instability` array in JSON reports, and an "Instability" summary card on every directory page in the HTML report ("—" when the directory has no boundary-crossing edges).
- **Stable Dependencies Principle violations** (#68): New `StabilityViolation` type and `analyzer::compute_stability_violations` detector. Flags cross-directory edges `from → to` where `I(from) < I(to)` — a more-stable directory depending on a less-stable one. Rendered as a `Stability Violations:` text section, a `stability_violations` array in JSON reports, and a root-page "Stability Violations" table in the HTML report.
- **Abstractness metric (Martin's A)** (#69 / PR #215): Per-directory `A = abstract / (abstract + concrete)` derived from counts of `trait` / `interface` / `abstract class` declarations versus `struct` / `enum` / `class` declarations. Wired end-to-end through scanner → analyzer → text/JSON/HTML reports. Supported languages: Rust (traits), Java (interfaces, abstract classes, records), Kotlin (interfaces and abstract classes), TypeScript + TSX (interfaces, abstract classes, enums). In-memory only — no SQLite schema change; type counts are recomputed from current source files at audit time via `scanner::recompute_type_counts`.
- **`audit_with_settings` now takes `type_counts: &[ModuleTypeCounts]`** (PR #215): Required for the abstractness pipeline. Caller sites in `commands/{audit,baseline,report,trend}.rs` were updated accordingly. Historical trend snapshots pass `&[]` since their source can't be reliably reconstructed.

### Changed

- **`TypeCounts` and `ModuleTypeCounts` moved to `core`**: The abstractness work in #215 introduced 4 layer violations by importing scanner-layer types into the analyzer layer. Moved both types to `core::{TypeCounts, ModuleTypeCounts}` so the analyzer can consume them without crossing layers. `scanner::parsers::TypeCounts` and `scanner::ModuleTypeCounts` remain available as re-exports. Brings the project's own health score back to 100/100.
- **`AuditResultBuilder` for test construction**: New test-only builder in `analyzer/mod.rs` with `with_*` methods and sensible defaults (empty vecs, `score: 100.0`). Existing tests construct ~25-field literals by hand; the builder lets each test specify only what it asserts on. Converted ~37 sites across `baseline.rs`, `reporter/{mod,html,graph}.rs`. The per-new-metric test-fixture tax (which was ~20 mechanical edits in #215 and #217) drops to one default in the builder.
- **`LayerIndex` consolidates layer-pattern matching**: `check_layer_rules`, `AuditResult::filter_by_layers`, and `AuditResult::apply_layer_weights` each previously compiled their own glob matchers and walked them independently. They now share a single `LayerIndex` in `analyzer/layers.rs` that compiles globs once and answers `layer_of(path)`. Also drops a `pattern.contains(extract_dir(path))` substring-fallback in `apply_layer_weights` that wasn't matched by any test and didn't have a documented use case.
- **`llms.txt` moved from repo root to `docs/llms.txt`**: GitHub Pages is configured to serve from `main:/docs`, so the file at the repo root wasn't reachable at the spec-defined URL `https://pererikbergman.github.io/noupling/llms.txt` (returned 404). Moving it into `docs/` makes it accessible at that canonical URL, which is where LLM tooling probes per the [llmstxt.org](https://llmstxt.org) convention.

### Fixed

- **Layer-violation detection silently ignored unlayered targets** (#220): `analyzer::check_layer_rules` only flagged a violation when both endpoints of an import were assigned to a layer. When the target file didn't match any layer pattern, the dependency was silently dropped from analysis — even when the source was in a layer. The whole point of the layers config is to surface deliberate dependency choices, but the previous behaviour hid exactly the most interesting case: a layered file importing a top-level cross-cutting file (config, settings, baseline helpers). Detection is now asymmetric: layered source → unlayered target is reported as a layer violation with `to_layer = "<unlayered>"`, prompting the team to either layer the target or record the exception in `dependency_rules`. Unlayered source → layered target stays silent (entrypoint case — `main.rs` importing analyzer is normal). **Migration**: existing projects with a `layers` config may see new violations after upgrading. Resolve by either widening layer patterns to cover the targets, adding a new layer for cross-cutting files, or explicitly allowing the dependency in `dependency_rules`. Self-applied: noupling's own `.noupling/settings.json` now layers `**/settings.rs` at the bottom of the stack.

- **Import resolvers substring-matched stdlib imports to project files** (#212 / PR #214): `path.ends_with(candidate)` was used as the resolution check in 12 language parsers, which matches at arbitrary byte boundaries rather than path segment boundaries. The user-visible symptom was `import re` in `stages/structure.py` producing a phantom `structure.py → structure.py` self-edge that triggered `from == to` rules — but the same class of bug was latent everywhere: `import os` collided with `chaos.py`, `import io` with `audio.py`, and so on, silently inflating hotspot/fan-in counts on any project with short-suffix filename collisions. Fixed via a new `ends_with_segment(path, candidate)` helper in `scanner/parsers/mod.rs` that anchors on `/` boundaries, applied across all affected parsers. Added a defence-in-depth self-edge filter at the scanner layer so any future resolver regression that produces `from == to` deps gets dropped before reaching the graph.

## [0.7.0] - 2026-05-10

This release is dominated by an **architectural deepening pass**: the codebase was reorganized for locality and testability without changing behavior. Two new languages (Elixir, Scala) bring the total to **16 supported**. Snapshot metadata is now fully in SQLite (no more JSON sidecars).

### Added

- **Elixir parser** (#89 / PR #206): `.ex` and `.exs` files via `tree-sitter-elixir`. Captures `alias`, `import`, `use`, and `require` directives.
- **Scala parser** (#88 / PR #206): `.scala` and `.sc` files via `tree-sitter-scala`. Captures `import` declarations and grouped imports.
- **`llms.txt`** (#141 / PR #207): AI-friendly project entry point at the repo root, following the [llmstxt.org](https://llmstxt.org) spec.
- **CLI integration test suite** (#200 / PR #205): 4 new end-to-end tests in `tests/cli.rs` covering `--fail-below` exit codes and report-format contracts. Total: 5 integration tests, 205 tests overall.
- **Opt-in pre-commit hook** (#201 / PR #204): `.githooks/pre-commit` runs `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings` to catch issues before CI. Enable with `git config core.hooksPath .githooks`.

### Changed

- **Architectural refactor: analyzer split** (#190 / PR #195): `src/analyzer/mod.rs` (was 2,955 LOC) split into 15 focused files — `coupling`, `cycles`, `direction`, `metrics`, `cohesion`, `independence`, `gravity_wells`, `red_flags`, `layers`, `rules`, `violation_age`, `critical_path`, `actions`, `monorepo`, `tests`. `mod.rs` is now the orchestrator.
- **`audit_with_settings` canonical seam** (#187 / PR #192): The 5-step `apply_*`/`filter_*` pipeline is now encapsulated in `analyzer::audit_with_settings(modules, deps, &settings)`. Command handlers call this once instead of spelling out the sequence themselves.
- **`LanguageParser` trait + per-language adapter files** (#189 / PR #194): `src/scanner/parser.rs` and `src/scanner/resolver.rs` deleted. Each language now lives in `src/scanner/parsers/<lang>.rs` (16 files after #206). Adding a language requires one new file and one line in `parsers/mod.rs::registry()`.
- **Thin command handlers** (#191 / PR #196): `src/main.rs` reduced from 988 LOC to 72 LOC (pure dispatch). Each command has its own handler in `src/commands/{init,scan,audit,trend,report,baseline,hook}.rs`.
- **`DependencyDirection` moved to `src/analyzer/direction.rs`** (#197 / PR #198): Previously defined in `src/core/mod.rs`. Re-exported as `analyzer::DependencyDirection`; call sites are unchanged.
- **Documentation refresh** (#199 / PR #203): `docs/architecture.md` and `CHANGELOG.md` updated to reflect the post-refactor structure.
- **CI architecture-audit threshold raised from 80 → 95** (PR #202): `.github/workflows/noupling-pr.yml` now fails any PR that drops the project's own health score below 95.

### Removed

- **JSON sidecar files in `.noupling/`** (#188 / PR #193): `diff-meta.json`, `suppressed.json`, and `external.json` are no longer written. Their data lives in SQLite alongside the snapshot: new `suppressed_count`, `diff_base`, `diff_changed_files` columns on the `snapshots` table; new `snapshot_external_deps` table. Existing databases are migrated forward via `ALTER TABLE … ADD COLUMN` (no-op if columns already exist).

### Fixed

- **Redundant `use serde_json;`** in `src/core/mod.rs` test module (PR #202). Resolves the only remaining `clippy::single_component_path_imports` warning. `cargo clippy --all-targets` is now warning-free.

## [0.6.0] - 2026-04-19

### Added

- **Software Dependency Risk Framework** (#167): Risk-weighted scoring replaces depth-based severity
  - `DependencyDirection` classification: Downward (weight 2), Sibling (weight 4), Upward (weight 6), Circular (weight 10)
  - RRI (Relationship Risk Index): direction_weight × density per violation
  - TRI (Total Risk Index): sum of all violation RRIs, derives health score
  - Configurable `risk_weights` in settings.json
- **Gravity Well detection** (#171): Identifies "God Object" modules with disproportionately high aggregate RRI
- **Architectural Red Flags** (#172): Fused Sibling (high-density sibling pairs) and Trapped Child (upward dependency) detection
- **External dependency tracking**: Scanner counts unresolved imports as third-party dependencies, surfaced in audit and reports
- **Transitive dependency direction** (weight 9) added to `DependencyDirection` for indirect dependencies through intermediate modules
- **Layer-specific thresholds**: `allow_sibling`, `max_sibling_density`, `reduced_sibling_weight` per layer in settings.json
- **Per-level cycle visualization** in bundle sunburst: cycle participants turn red, weakest hop highlighted with "break this side" tooltip
- **N-cycle detection**: 3+ node cycles (A→B→C→A) now detected via DFS fallback when no mutual pairs exist in SCC
- **Metrics Guide**: Expandable guide in HTML report and section in MD reports explaining all metrics
- Direction badges (↓↔↑↻) and RRI values shown in all report formats
- TRI metric in HTML project banner, bundle subtitle, PR summary, and briefing header
- Gravity Wells and Red Flags sections in CLI, JSON, XML, MD, and HTML reports
- RRI-based Sonar severity mapping (INFO/MINOR/MAJOR/CRITICAL/BLOCKER)
- Dashed red edges for circular deps in Mermaid and DOT graphs

### Changed

- Default `coupling_mode` changed from "actionable" to "strict" — sibling coupling now affects the score by default
- Health score formula: `100 × (1 - TRI / (total_modules × max_weight))` replaces old `100 × (1 - sum_severity / total_modules)`
- `coupling_mode` can now be set at top level of settings.json (not just inside thresholds)

## [0.3.0] - 2026-04-15

### Added

- **Dependency graph diagrams**: `--format mermaid` and `--format dot` with color-coded nodes (green/yellow/red).
- **Dependency weight**: Import count per directory pair multiplies severity. Shows `x3` for weighted violations.
- **Trend tracking**: `noupling trend .` shows score history across snapshots with delta.
- **Custom dependency rules**: Define `dependency_rules` in settings.json with glob patterns to forbid specific imports.
- **Version in reports**: All report formats now include `Generated by noupling v0.3.0`.
- **Dev install script**: `./scripts/install-dev.sh` installs as `noupling-dev` alongside production.

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
