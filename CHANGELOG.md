# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.8.0] - 2026-09-04

This release ships the **Explorer** — a new interactive HTML report format for reading an architecture, not just scoring it — on top of a **Cargo workspace split** into `noupling-core` / `noupling` (cli) / `noupling-explorer`. It also completes Robert C. Martin's package-metric trio (Abstractness, Instability, Distance from Main Sequence), reworks cohesion around logical nodes, and closes a deepening pass over the reporter and Explorer state. Distribution is now Homebrew + GitHub Releases only.

**Upgrade notes:** if you build from source, use `cargo install --path crates/noupling-cli` (the root is now a virtual workspace). Projects with a `layers` config may see new `<unlayered>` violations (#220). JSON consumers must handle `"cohesion": null` and the new `kind` field on cohesion entries (#224).

### Added

- **Explorer report format — `noupling report --format explorer`** (#228 epic, #231–#300): A new interactive, single-file HTML report built for *reading* an architecture rather than skimming a score. The Rust side (`noupling-explorer` crate) serializes a versioned Data Contract — modules, edges, cycles, violations, gravity wells, red flags, per-file LOC / Ca / Ce / I / blast radius, score breakdown, and up to 12 snapshots of history — and embeds it into a React + Tailwind template (`crates/noupling-explorer/template/`, built with Vite into one self-contained file that CI drift-checks). What's in the box:
  - **Layered Structure Map (LSM)** — the primary canvas. Packages and files laid out by layer with edges drawn between them; violations, cycles, and selected edges get distinct accents. Click to select, double-click to drill in, breadcrumb to scope back out. Singleton chains collapse; unlayered codebases get synthetic tiers so the map still reads.
  - **Auto-detected layers** (#260, #267): when `.noupling/settings.json` declares no `layers`, the Explorer infers them from directory names and switches coupling into an actionable mode. A banner explains what was inferred and offers a copy-to-clipboard settings snippet (#266).
  - **Side panel tabs** — *Info* (score block front and centre, #272), *Issues* (every violation / cycle / gravity well / red flag in scope as an actionable list, with focus mode that scopes the canvas to the participants, #265/#275), *Rules* (click a broken rule to filter, click again to jump to the first offender, #268/#269), *Files* (expand-on-click tree with in-tab breadcrumb and folders-only filter, #259/#273), *Levels* (Finder-style one-level drill-down, #274).
  - **Details inlay** (#235, #293): selecting a node or edge opens an inlay column — not a floating pane — with metrics, an "About this verdict" explainer (#276), cycle subtitles that show the break edge with weight contrast (#277), and **click-to-source** links honouring `--editor vscode|jetbrains|sublime|cursor`.
  - **Search, spot filters, layer overlay, cycle highlight** (#237–#240), keyboard navigation and accessibility pass (#242), a field-guide dialog and tooltips on under-explained controls (#300).
  - **Alternate views** — *Matrix* (dependency-structure matrix bounded at ≤200 nodes, clickable cells and labels, #262/#263/#298/#299), *Force* (d3-force layout with Rust-precomputed cluster boundaries, #278), *Composition* (derived-only v1 per PRD §10.8, #279), plus **path finder** and **min-cut** overlays (#262).
  - **Score breakdown dialog** (#270) and a **history scrubber** showing the health-score trend over snapshots (#271; `--no-history` drops the block to shrink the file).
  - **LLM enrichment v1** (#280): optional per-module summaries read from `.noupling/enrichment/modules.json` and merged into the contract when present.
  - New `report` flags: `--output <FILE>`, `--editor <EDITOR>`, `--title <STRING>`, `--no-history` (explorer-only). Playwright smoke suite runs in CI; a manual regression suite targets real codebases (#261).

  Known gaps are tracked in #315 and will land as follow-ups; this is the v1 "interactive readme" from PRD §8, with v2 sandbox editing and the remaining v3 views still to come.

- **Distance from Main Sequence (Martin's D)** (#70): Per-directory `D = |A + I − 1|`, joining the abstractness (#69) and instability (#68) metrics. `D = 0` sits on the main sequence (well-balanced); high `D` lands in one of two danger zones. Modules with low instability + low abstractness are flagged as **Zone of Pain** (stable + concrete, rigid against change); modules with high instability + high abstractness are flagged as **Zone of Uselessness** (abstract + unstable, speculative architecture nobody uses). Threshold for danger-zone classification is `D > 0.5` — anything more than halfway off the main sequence. Surfaced as a `Distance from Main Sequence:` text section, a `distance` array in JSON reports (with `zone: "main_sequence" | "zone_of_pain" | "zone_of_uselessness"`), and a root-page table in the HTML report. Closes the Martin metric arc (A + I + D together).
- **Per-directory instability metric (Martin's I)** (#68): `I = Ce / (Ca + Ce)` per directory, derived from cross-directory dependency edges. Per-file instability already existed on `ModuleMetrics` (used by the dashboard and the text "Zone of Pain" section); the new aggregate operates at the package level, which is where Martin's metric is conventionally applied and where it composes with abstractness for the Distance from Main Sequence (#70). Internal (same-directory) edges are excluded. Surfaced as a new `Instability:` text section, an `instability` array in JSON reports, and an "Instability" summary card on every directory page in the HTML report ("—" when the directory has no boundary-crossing edges).
- **Stable Dependencies Principle violations** (#68): New `StabilityViolation` type and `analyzer::compute_stability_violations` detector. Flags cross-directory edges `from → to` where `I(from) < I(to)` — a more-stable directory depending on a less-stable one. Rendered as a `Stability Violations:` text section, a `stability_violations` array in JSON reports, and a root-page "Stability Violations" table in the HTML report.
- **Abstractness metric (Martin's A)** (#69 / PR #215): Per-directory `A = abstract / (abstract + concrete)` derived from counts of `trait` / `interface` / `abstract class` declarations versus `struct` / `enum` / `class` declarations. Wired end-to-end through scanner → analyzer → text/JSON/HTML reports. Supported languages: Rust (traits), Java (interfaces, abstract classes, records), Kotlin (interfaces and abstract classes), TypeScript + TSX (interfaces, abstract classes, enums). In-memory only — no SQLite schema change; type counts are recomputed from current source files at audit time via `scanner::recompute_type_counts`.
- **`audit_with_settings` now takes `type_counts: &[ModuleTypeCounts]`** (PR #215): Required for the abstractness pipeline. Caller sites in `commands/{audit,baseline,report,trend}.rs` were updated accordingly. Historical trend snapshots pass `&[]` since their source can't be reliably reconstructed.

### Changed

- **Distribution: Homebrew + GitHub Releases only, crates.io dropped** (closes the open question from #229): The workspace split left `cargo publish` unworkable without versioned inter-crate deps, and nobody installs noupling through crates.io. Rather than maintain a three-crate publish order, all crates now carry `publish = false` (set once in `[workspace.package]`, inherited via `publish.workspace = true`). Supported install paths are `brew install pererikbergman/noupling/noupling`, the prebuilt binaries on GitHub Releases, and building from source with `cargo install --path crates/noupling-cli`. README and the docs site were updated accordingly — the old `cargo install --path .` fails on the virtual workspace manifest.

- **Reporter: widen `ReportFormatter` seam to cover every non-bespoke format** (#317): The `ReportFormatter` trait + registry shipped in #301 only carried the six "string + write + print" simple formats (json/xml/sonar/mermaid/dot/briefing). Issue #317 widens `FormatterContext` (now carries `deps`, `settings`, `prev_score`, `prev_violation_count`) and introduces an `Output` enum (`SingleFile { … }` for adapters that produce one file, `Directory { … }` for adapters that write their own tree). Five more formats register as adapters: **md, html, bundle, dashboard, pr**. `commands/report.rs` drops 90 lines of bespoke `match` arms and an `all`-specific helper; the `all` arm now enumerates the registry so a new adapter participates automatically. Only **strategy** and **explorer** keep bespoke arms: strategy needs the `SnapshotRepository`/`ModuleRepository`/`DependencyRepository` triad for its history walk, and explorer carries an option struct (editor, title, no-history, override output path, LLM enrichment, auto-detected layers, …) that would balloon `FormatterContext` for no gain. All 308 tests + 6 new registry tests green; clippy `-D warnings` green; `noupling report --format all .` produces 12 reports (was 12) with byte-identical output. Pairs with #319 (the canonical `Report` shape) — that's the foundation this widening builds on.

- **Explorer: introduce `HighlightPolicy` behind `CanvasArea`** (#318): The eight highlight-related props CanvasArea used to forward verbatim (`pathHighlight`, `minCutHighlight`, `cyclesByNode`, `selectedEdge`, `expandedContainers`, `participantFiles`, `layerOverlay`, `cycleHighlight`) collapse into one `highlight: HighlightPolicy` value built once in `App.tsx`. The policy exposes the resolved edge accent via one method — `edgeAccent(from, to, isViolation, isCycle) → "selected" | "path" | "minCut" | "violation" | "cycle" | "default"` — so the LSM's `EdgePath` stops carrying the precedence rule (six nested ternaries reduce to a record lookup) and so a new accent kind only needs the policy module touched. Matrix takes the policy too for parity; it reads `highlight.selectedEdge` directly. `CanvasArea` prop count drops from 26 to 18. No behaviour change; all 30 Playwright smoke specs pass.

- **Explorer: introduce `state/queries.ts` so tabs + panels stop touching raw `DataContract` arrays** (#320): Eighteen call sites across `DetailsPanel`, `EdgeDetailsPanel`, `SidePanel`, `IssuesTab`, `RulesTab`, `FilesTab`, `LevelsTab`, and `CanvasArea` re-derived the same membership predicates inline (`data.cycles.filter(c => c.members.includes(id))`, `data.edges.filter(e => e.to === id)`, `data.violations.find(...)`, etc.). They now call named queries — `cyclesInvolving`, `incomingOf`, `outgoingOf`, `violationsFor`, `gravityWellFor`, `redFlagsForModule`, `nodeById`, `buildChildIndex`, `cycleMembershipCounts`, `firstViolationForRule`, plus list queries `allViolations` / `allCycles` / `allGravityWells` / `allRedFlags` and the convenience `totalIssueCount` — exported from `state/queries.ts`. The contract shape is now consumed in exactly one module, so a future Rust-side rename (or v3's history-scrubber rewiring) follows in one place instead of eighteen. `App.tsx`, `Matrix.tsx`, and `lsm/layout.ts` keep direct array access — they're data-shaping infrastructure, not consumers, and pushing them through queries would only mask the layout-build seam. No behaviour change; all 30 Playwright smoke specs still pass.
- **Reporter: carve `data::Report` (`JsonReport`) into its own module + split each format out** (#319): `reporter/mod.rs` used to hold the canonical `JsonReport` shape, its 13 sub-structs, the `from_audit` builder, the `build_json_dir_tree` walker, two path helpers, *and* three formats (`format_xml`, `format_sonar`, `format_pr`) *and* the test suite — 2028 LOC. Issue #319 carves the wire shape into `reporter/data.rs` (562 LOC, the canonical `Report`), and splits each format into its own sibling: `xml.rs` (142 LOC), `sonar.rs` (124 LOC), `text.rs` (382 LOC — owns `format_text` and `format_monorepo_text`), `pr.rs` (117 LOC), `briefing.rs` (136 LOC). `mod.rs` becomes a thin re-export shell — ~50 LOC of production code plus the pre-existing test suite. No external import paths change: `reporter::JsonReport`, `reporter::format_xml`, `reporter::format_sonar`, `reporter::format_text`, `reporter::format_monorepo_text`, `reporter::format_pr`, `reporter::format_briefing` all keep working through unchanged re-exports. Pairs with #317 (widen the `ReportFormatter` seam): once the canonical `Report` shape is its own module, folding md/html/bundle/dashboard/explorer/strategy adapters into the same registry as xml/sonar/text/pr/briefing becomes a mechanical follow-up. All 308 tests green; clippy `-D warnings` green; noupling's own audit unchanged at 98.3.
- **Explorer: extract `IssueFocus` module from `App.tsx`** (#316): The canvas-level focus state driven by Issues-tab selections — `participantsOf`, `longestCommonAncestor`, expanded-container derivation, and the inter-participant edge set — moves into `crates/noupling-explorer/template/src/state/issueFocus.ts` behind one public function `computeIssueFocus(issue, key, data) → IssueFocus`. `App.tsx`'s `onIssueFocus` handler shrinks from ~50 inline lines to a four-line call. Pure function, no React, no setState; callers still own the side effect of scoping the view to `focus.lca`. Unit-test framework is not yet wired in the template subproject (Playwright e2e only), so verification is the existing smoke suite — all 30 specs still pass. Lays groundwork for #318 (HighlightPolicy) where the focus rule becomes one input to a unified canvas-highlight policy instead of a tangled sibling of `pathFinder` and `minCut` memos.

- **Workspace split: `noupling-core` / `noupling` (cli) / `noupling-explorer`** (#229, Task 1 of #228): The single-crate repo became a Cargo workspace with three crates under `crates/`. `noupling-core` is the pure analysis library (analyzer, scanner, storage, layers/rules, settings, baseline, diff). The cli crate (package name still `noupling`, binary still `noupling`) is the only consumer that wires reporter + cli code; it depends on `noupling-core` and `noupling-explorer`. `noupling-explorer` is an empty stub crate ready for Task 2 of #228. Dependency direction is strictly one-way: `cli → explorer → core`, `cli → core`. The boundary is dogfooded in `.noupling/settings.json` via new `dependency_rules` that forbid core → cli/explorer and explorer → cli/reporter; the same boundary is enforced at compile time by the workspace layout. The `analyzer::AuditResultBuilder` test helper is now exposed via a `test-utils` cargo feature on `noupling-core`, enabled as a dev-dependency feature on the cli crate. CI (`cargo check/clippy/test`) switched to `--workspace` and `cargo fmt` switched to `--all`. `scripts/release.sh` likewise. Binary surface is unchanged (`noupling --help` byte-identical, all integration tests pass without modification). Both follow-ups flagged in #229 are now closed out: the Homebrew tap formula builds from `crates/noupling-cli` (falling back to the root for pre-workspace tags), and crates.io publishing is dropped (see below).

- **Cohesion: logical-node algorithm via `LogicalNodeIndex` primitive** (#225, slice 2 of #223): The cohesion value algorithm now follows the model documented in `docs/dependency-graph.md` § Analysis Step 2. For every Package directory, each direct tree-child — file or subdirectory — counts as one logical node. Cohesion is the fraction of file-level edges that cross between different logical-node children, over `n × (n − 1)`. Subdirectories are *opaque*: an edge fully inside a subdirectory (e.g. `scanner/x.rs → scanner/y.rs`) does not contribute to the parent's cohesion (it contributes to `scanner/`'s own cohesion at its own ply). Containers continue to report `cohesion: None`. The change is implemented behind a new `LogicalNodeIndex` deep module (in-crate, `pub(super)`) — a per-directory index from `&[Module]` that answers `kind(dir)`, `children(dir)`, and `logical_node_of(file_id, dir)`. Future analyses needing the same tree-overlay view (D_acc aggregation, coupling-between-subdirs) can reuse the primitive without re-implementing the grouping logic. The reported cohesion *value* for many directories changes — most directories that previously read `0.0` or were filtered out will now show a meaningful cohesion reflecting their true cross-area coupling.
- **Cohesion: Container/Package classification + `Option<f64>` shape** (#224, slice 1 of #223): `CohesionMetrics` now carries a `kind` field (`Container` for directories with only subdirectories, `Package` for directories with ≥1 direct file) and `cohesion: Option<f64>` (`None` for Containers, `Some(value)` for Packages). The `cohesion` array in `AuditResult` now contains every directory in the project — both kinds — sorted Packages-first by cohesion ascending, then Containers alphabetically. The text reporter's `Low Cohesion:` section now lists only Packages; Containers (grouping folders like `src/features/`) are no longer mis-flagged as low-cohesion. The JSON report gains a new `cohesion` array with the same shape, where Containers serialize as `"kind": "Container", "cohesion": null`. The cohesion value algorithm for Packages is unchanged in this PR (still file ↔ file edges among direct files); slice 2 (#225) widens it to the logical-node rule documented in `docs/dependency-graph.md` § Analysis Step 2. **Downstream JSON consumers must handle `"cohesion": null` and the new `kind` field.**
- **`TypeCounts` and `ModuleTypeCounts` moved to `core`**: The abstractness work in #215 introduced 4 layer violations by importing scanner-layer types into the analyzer layer. Moved both types to `core::{TypeCounts, ModuleTypeCounts}` so the analyzer can consume them without crossing layers. `scanner::parsers::TypeCounts` and `scanner::ModuleTypeCounts` remain available as re-exports. Brings the project's own health score back to 100/100.
- **`AuditResultBuilder` for test construction**: New test-only builder in `analyzer/mod.rs` with `with_*` methods and sensible defaults (empty vecs, `score: 100.0`). Existing tests construct ~25-field literals by hand; the builder lets each test specify only what it asserts on. Converted ~37 sites across `baseline.rs`, `reporter/{mod,html,graph}.rs`. The per-new-metric test-fixture tax (which was ~20 mechanical edits in #215 and #217) drops to one default in the builder.
- **`LayerIndex` consolidates layer-pattern matching**: `check_layer_rules`, `AuditResult::filter_by_layers`, and `AuditResult::apply_layer_weights` each previously compiled their own glob matchers and walked them independently. They now share a single `LayerIndex` in `analyzer/layers.rs` that compiles globs once and answers `layer_of(path)`. Also drops a `pattern.contains(extract_dir(path))` substring-fallback in `apply_layer_weights` that wasn't matched by any test and didn't have a documented use case.
- **`llms.txt` moved from repo root to `docs/llms.txt`**: GitHub Pages is configured to serve from `main:/docs`, so the file at the repo root wasn't reachable at the spec-defined URL `https://pererikbergman.github.io/noupling/llms.txt` (returned 404). Moving it into `docs/` makes it accessible at that canonical URL, which is where LLM tooling probes per the [llmstxt.org](https://llmstxt.org) convention.

- **Architectural deepening pass** (#301–#308, PRs #309–#314): Six seams were deepened without behaviour change — the audit pipeline folded into one module (#304); `DatabaseSession` widened so callers stop reaching for `find_db` (#308); a `ReportFormatter` trait + registry for the simple report formats (#301, later widened in #317); a `ContractBuilder` for staged Data Contract construction (#303); `explorerState` gains `setState(patch)` with config-driven persistence (#302); and `SidePanel` split into a slim router plus five per-tab modules (#306).
- **CI: `release.yml` now chains into `homebrew.yml` via `repository_dispatch`** (#210 / PR #211): `GITHUB_TOKEN`-triggered `release: published` events deliberately don't chain, so the release workflow fires an explicit `release-published` dispatch after the binary build. The Homebrew tap formula bumps itself without a manual step.
- **Release hygiene**: main's CI went red on rustfmt drift and on new clippy lints in rustc 1.98 (`format_in_format_args`, `new_without_default`, `items_after_test_module`, `double_ended_iterator_last`); all fixed (#328, #330). `cargo clippy --workspace --all-targets -- -D warnings` is clean again, matching the opt-in pre-commit hook.
- **Inter-crate deps no longer pin `version`**: with crates.io dropped, `noupling-cli → noupling-core/-explorer` and `noupling-explorer → noupling-core` are plain `path` deps, so bumping `[workspace.package] version` is the only edit a release needs.

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
