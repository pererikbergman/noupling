# Noupling Explorer Report — Product Requirements Document

**Status:** Draft 2
**Owner:** Per Erik Bergman
**Author handoff target:** Noupling implementation agent

---

## 1. Overview

**The Explorer is a new noupling report format.** When a user runs `noupling report --format explorer /path/to/code`, noupling emits a single self-contained HTML file (`noupling-explorer.html`) that the user opens in a browser. The file contains everything needed to navigate, understand, and reason about the codebase noupling just scanned — visualizations, metrics, layer rules, drill-down navigation, search, and (in later milestones) in-memory refactoring experiments.

**Mental model:** The Explorer is the **interactive readme noupling generates for your codebase**. It is not a CI gate, not a violation list, not a dashboard pinned to a wall. It is a *learning surface* — a generated artifact you open when you want to understand a project (your own or someone else's) at the architectural level.

**Why this exists:** Noupling already computes everything a developer needs to understand a codebase's structure (Layers, coupling metrics, cycles, blast radius, gravity wells, red flags, distance from main sequence). Today this surfaces through reports that are *issue-first*: `html` is a diggable list of violations and hotspots; `dashboard` is a leader-facing summary of where things are going wrong. Both answer "what's broken?" — but neither shows the **whole codebase**, including the parts that are healthy.

The Explorer is **whole-codebase-first**. Healthy modules, clean Layers, and zero-violation Packages are as visible as hotspots. A developer opens it to build a *mental model* of the project — to learn the architecture, not to triage it. Replacing `html` or `dashboard` is **not a goal of this PRD**; the Explorer earns replacement (if at all) only by becoming demonstrably broader *and* as easy to dig into as the existing reports. That decision belongs to a future PRD.

**Audience:** Developers — both the ones using noupling on their own code (already comfortable with the CLI) and those onboarding to an unfamiliar repository. Not non-technical stakeholders (use the existing `dashboard` report for that).

---

## 2. Goals & Non-Goals

### 2.1 Goals

- **G1.** Make a noupling-scanned codebase *learnable* — a developer opens the Explorer and within 30 seconds knows the project's shape, layers, and structural hot spots without reading source.
- **G2.** Reuse 100% of noupling's existing analysis output. No new data extraction work for v1.
- **G3.** Ship as a single self-contained HTML file that is shareable, emailable, hostable as a static site, and openable via `file://`.
- **G4.** Honor noupling's existing architecture and conventions — Rust generates the HTML using the same patterns as other reporters; no second toolchain, no npm, no JavaScript framework dependency.
- **G5.** Provide click-through navigation to source code via `vscode://file/<path>` URLs (and configurable equivalents for other editors).
- **G6.** Support in-memory exploration of "what if I refactored this?" scenarios with live metric recomputation (v2). The Explorer never writes to the filesystem.
- **G7.** Show the whole codebase, not just its problems. Healthy modules, clean Layers, and zero-violation Packages must be as visible as hotspots — building a mental model requires seeing what's *right*, not only what's *wrong*. This is the property that distinguishes the Explorer from `html` and `dashboard`.

### 2.2 Non-Goals

- **NG1.** Editing `.noupling/settings.json` rules from the Explorer. Configuration changes happen in the user's editor; the Explorer reflects the current `.noupling/settings.json` and recomputes when the user re-runs noupling.
- **NG2.** Re-scanning the codebase from inside the Explorer. The page does not shell out, does not run noupling, and does not touch the filesystem. Re-scan happens by the user re-running `noupling report --format explorer`.
- **NG3.** Real-time file watching. The Explorer is a snapshot of the scan at generation time.
- **NG4.** Picking a folder from inside the Explorer. The folder is picked by `noupling scan`.
- **NG5.** Being a CI gate or violation list. That is `noupling audit`'s job.
- **NG6.** Native desktop app shell (Tauri/Electron). The browser is the host.
- **NG7.** Multi-user collaboration, comments, shared state. The Explorer is single-user.
- **NG8.** Server-side persistence. The user manually downloads any session state (action plans) if they want to keep it.
- **NG9.** Replacing or deprecating the existing `html` or `dashboard` reports. They serve a different purpose (issue digging; leader-facing summary). The Explorer earns replacement (if at all) by becoming demonstrably better and as easy to use — and that's a decision for a future PRD, not this one.

---

## 3. Users & Use Cases

### 3.1 Persona A — The Onboarding Developer

> "I just joined this team, I have to understand 80k lines of Rust by Friday, and the README is three sentences long."

**Jobs to be done:**
- Get a one-screen overview of the codebase's modules and how they relate.
- Identify which modules are central (high fan-in) vs leaf (low coupling).
- See which Layers exist and how strictly they're separated.
- Find the entry points and the "hot" modules to start reading.
- Jump from a node to the relevant source file in their editor.

### 3.2 Persona B — The Architect Reviewing Existing System

> "I inherited this 5-year-old service. Where are the cycles? What's holding the architecture back?"

**Jobs to be done:**
- See the Layered Structure Map at a glance.
- Find cycles immediately; understand which references would break them.
- Spot Gravity Wells (modules with too much inbound coupling).
- Compare actual structure against intended Layers.
- Pick refactoring priorities by health-score impact / violation severity.

### 3.3 Persona C — The Developer Considering a Refactor

> "I want to move this module from `internals` to its own `api` layer. What breaks? What gets better?"

**Jobs to be done:**
- Make virtual moves in the Explorer (drag a module to a different parent).
- See live: new violations created, old violations cleared, health-score delta, instability changes.
- Iterate on the refactor plan without touching source.
- Export the plan as a checklist (action plan) when they're ready to do the real work in their editor.

(v2 scope — drives the in-memory refactoring feature.)

---

## 4. Product Principles

- **P1. Read-only by default, in-memory experimental.** The Explorer never writes to the filesystem. Refactoring experiments are in-memory; persistence is an explicit manual download.
- **P2. Single artifact, self-contained.** One HTML file. No fetch calls. No CDN. Inlined data, inlined assets. Works opened from `file://`.
- **P3. Comprehension over enforcement.** Surface architectural facts in a way that builds understanding. Violations are *one* data point, not the headline.
- **P4. Reuse noupling primitives.** Anything noupling already computes (metrics, Layers, cycles, blast radius, snapshots) feeds the Explorer untouched. Net-new analysis is deferred until proven necessary.
- **P5. Rust-native generation.** The Explorer report is a new variant under `src/reporter/`. Same conventions as the existing HTML/dashboard/sunburst reporters. No JavaScript framework dependency, no npm toolchain.
- **P6. Editor as the canonical edit surface.** Configuration changes (`.noupling/settings.json`) and source edits happen in the user's editor. The Explorer is for navigating and thinking; the editor is for changing.
- **P7. Phase by demonstrated need.** v1 ships the smallest possible view-only experience. Each later milestone is justified by evidence the previous one is in active use and falling short on specific axes.

---

## 5. Architecture & Technical Stack

### 5.1 Same repo, strict crate boundaries

The Explorer lives in the same repo as noupling, but as a **separate crate inside a Cargo workspace**. This gives compile-time isolation: the Explorer cannot see analyzer/scanner/storage internals, and the rest of noupling cannot see Explorer internals. The same property that noupling itself exists to enforce, applied to noupling's own source tree.

The single-crate layout the project ships today is restructured into a workspace:

```
noupling/                        (Cargo workspace root)
├── Cargo.toml                   (workspace manifest)
└── crates/
    ├── noupling-core/           ← NEW — pure analysis library (graph, metrics, layers, rules, snapshots)
    │   └── src/lib.rs           ← public API surface; no CLI, no I/O concerns beyond pure analysis
    ├── noupling-cli/            ← the existing binary, depends on noupling-core
    │   └── src/
    │       ├── main.rs
    │       └── reporter/        ← existing reporters (html, dashboard, bundle, graph, md, strategy)
    └── noupling-explorer/       ← NEW — Explorer reporter, isolated crate
        └── src/
            ├── lib.rs           ← single public entry point, consumed by noupling-cli
            ├── data.rs          ← serializes via noupling-core's public API only
            ├── template.rs      ← assembles the HTML frame + inlined data + inlined assets
            └── assets/          ← inlined-at-build-time JS, CSS, SVG, optional WASM
```

Dependency direction is strictly one-way:

```
noupling-cli ──depends on──> noupling-explorer ──depends on──> noupling-core
        └───────────────────────depends on──────────────────────┘
```

- `noupling-core` has **zero** awareness of the Explorer or any reporter. Removing every reporter must leave `noupling-core` compiling.
- `noupling-explorer` depends **only** on `noupling-core`'s public API. It must not depend on `noupling-cli`, on other reporter modules, or on internal modules of `noupling-core` (no `pub(crate)` workarounds, no re-exporting private items).
- `noupling-cli` wires the Explorer into the `--format explorer` flag and is the only crate allowed to touch both sides.

`noupling report --format explorer <path>` writes `.noupling/explorer.html` (matching the convention noupling uses for other artifacts), or to `--output <dir>` if specified. The file is a fully self-contained HTML document.

### 5.1.1 Boundaries dogfooded via `.noupling/settings.json`

Compile-time isolation is the primary guarantee; noupling's own audit is the secondary one. The workspace's `.noupling/settings.json` gains:

- An `explorer` layer scoped to `**/crates/noupling-explorer/**`.
- `dependency_rules` that forbid any path under `**/crates/noupling-core/**` from importing reporter or Explorer paths, and forbid `**/crates/noupling-explorer/**` from importing `**/crates/noupling-cli/**` or reporter siblings.
- `noupling audit` on the noupling repo itself must pass with these rules in place. A breach is both a Rust compile error *and* an audit failure — the boundary cannot rot quietly.

### 5.2 Browser side (inside the HTML)

The interactive layer is **plain JavaScript** loaded inline. No React, no Vite, no npm. The pattern mirrors noupling's existing interactive HTML reports (which use vanilla JS + D3 where graph rendering is needed).

- Inline `<script>` blocks contain all interactivity (event handlers, search, filtering, view switching, drag interactions).
- Visualization library: if noupling already bundles **D3.js** for the sunburst/dashboard reports, reuse it. If not, use the lightest fit per view (D3 for force-directed and matrix; vanilla SVG/Canvas for LSM and simpler views).
- Inline `<style>` blocks contain all CSS.
- Inline `<script type="application/json" id="noupling-data">` block contains the scan data serialized per the data contract below.

### 5.3 WebAssembly (v2+)

For v2's in-memory virtual refactoring with live metric recompute, compile noupling's analysis library to `wasm32-unknown-unknown` and inline (base64) the WASM module into the HTML. Browser JS loads the WASM, builds an in-memory graph from the inlined JSON, and re-runs metric calculations on virtual edits.

**Implication for noupling's structure:** analysis logic must live in a library crate so it can target WASM. The lib/bin split (`noupling-core` / `noupling-cli` / `noupling-explorer`) is **already done in v1** as part of §5.1's strict-boundary requirement. v2 only adds the `wasm32-unknown-unknown` target to `noupling-core`; no further restructuring is needed.

### 5.4 Build-system changes

v1's build-side change is **the workspace split itself**: one `Cargo.toml` becomes a workspace manifest plus three crate manifests. No npm, no Vite, no wasm-pack, no new toolchain. `cargo build` still produces the same binary; CI commands are equivalent (`cargo build --workspace`, `cargo test --workspace`). The split is mechanical, not a rewrite — existing modules move under `crates/noupling-core/src/` and `crates/noupling-cli/src/` largely unchanged.

v2 adds `cargo build --target wasm32-unknown-unknown` for the WASM module; that's the only further build-side addition and it's bounded to v2.

---

## 6. Data Contract

The Explorer consumes a JSON document inlined into the HTML at generation time. The shape is a superset of what noupling already serializes for its other reports and is versioned for forward compatibility.

```json
{
  "format_version": 1,
  "noupling_version": "0.7.0",
  "generated_at": "2026-06-03T14:00:00Z",
  "codebase": {
    "path": "/Users/me/code/foo",
    "language_distribution": [
      { "language": "rust", "file_count": 142, "loc": 18432 },
      { "language": "typescript", "file_count": 78, "loc": 9210 }
    ],
    "module_count": 23,
    "file_count": 312,
    "edge_count": 1841
  },
  "health_score": 82,
  "summary_counts": {
    "violations": 7,
    "cycles": 2,
    "gravity_wells": 1,
    "red_flags": 3
  },
  "layers": [
    {
      "name": "reporter",
      "pattern": "**/reporter/**",
      "allow_sibling": false,
      "index": 0,
      "file_count": 12,
      "afferent": 0,
      "efferent": 41,
      "instability": 1.0
    },
    {
      "name": "analyzer",
      "pattern": "**/analyzer/**",
      "allow_sibling": false,
      "index": 3,
      "file_count": 19,
      "afferent": 8,
      "efferent": 14,
      "instability": 0.64
    }
  ],
  "dependency_rules": [
    {
      "from": "**/reporter/**",
      "to": "**/storage/**",
      "allow": false,
      "message": "Reporters consume audit results, not raw storage handles."
    }
  ],
  "effective_rules": [
    {
      "from": "**/reporter/**",
      "to": "**/storage/**",
      "allow": false,
      "message": "Reporters consume audit results, not raw storage handles.",
      "source": "dependency_rule",
      "current_violation_count": 0
    },
    {
      "from": "**/analyzer/**",
      "to": "**/reporter/**",
      "allow": false,
      "message": "Layer 'analyzer' (index 3) may not depend on layer 'reporter' (index 0) — layers flow downward.",
      "source": "layer_order",
      "current_violation_count": 0
    }
  ],
  "nodes": [
    {
      "id": "src/ui/Form.tsx",
      "kind": "file",
      "parent": "src/ui",
      "layer": "ui",
      "metrics": {
        "afferent": 0,
        "efferent": 12,
        "instability": 1.0,
        "loc": 142,
        "blast_radius_upstream": 0,
        "blast_radius_downstream": 12
      }
    }
  ],
  "edges": [
    {
      "from": "src/ui/Form.tsx",
      "to": "src/domain/payment.ts",
      "weight": 4,
      "violates_rule": null
    }
  ],
  "cycles": [
    {
      "id": "cycle-1",
      "size": 4,
      "members": ["src/a", "src/b", "src/c", "src/d"],
      "minimum_cut": [{ "from": "src/c", "to": "src/a" }]
    }
  ],
  "violations": [
    {
      "rule": { "from": "ui", "to": "database" },
      "edge": { "from": "src/ui/Form.tsx", "to": "src/infra/db.ts" },
      "severity": "high",
      "introduced_in": "2026-04-12"
    }
  ],
  "history": [
    { "snapshot_id": "abc123", "taken_at": "2026-05-30T12:00:00Z", "health_score": 78 }
  ]
}
```

**Stored vs derived.** `layers`, `dependency_rules`, and the `pattern`/`allow_sibling` fields come directly from `.noupling/settings.json` (the user's authored config). Everything else — `health_score`, `summary_counts`, per-layer `file_count`/`afferent`/`efferent`/`instability`, `effective_rules` (with `source` and `current_violation_count`), `nodes`, `edges`, `cycles`, `violations` — is computed at audit time from the latest snapshot. The Explorer does not need to know the distinction at render time, but v2's "edit this rule in your editor" link must point back to the settings file, not the derived data.

Fields are additive across versions. Older Explorers reading newer data ignore unknown keys; newer Explorers reading older data show "not available" for missing metrics.

---

## 7. Milestone Plan

The Explorer is delivered in three milestones. Each is shippable independently — v1 is useful with no v2 work; v2 adds value to v1 without invalidating it; v3 layers in advanced views.

| Milestone | Theme | Headline capability | Approximate scope |
|---|---|---|---|
| **v1** | **The Interactive Readme** | Open the Explorer, understand the codebase at a glance, drill down, jump to source. | View-only. ~2–3 weeks. |
| **v2** | **The Sandbox** | Drag a module elsewhere, see metrics recompute live, export an action plan. | Adds WASM + interaction. ~2–4 weeks on top of v1. |
| **v3** | **Advanced Views** | Dependency Matrix, force-directed graph, cycle composition browser, history scrubber. | Adds views. ~2–3 weeks on top of v2. |

Each section below details the features within a milestone with acceptance criteria.

---

## 8. v1 — The Interactive Readme

### 8.1 Goal

A developer opens the Explorer and within 30 seconds knows: what kind of codebase this is, what Layers exist, where the gravity is, which Layers are healthy vs strained, and where to start reading.

### 8.2 Feature: Codebase Header

The top of the page shows a concise summary of the scanned codebase.

- **F1.1** Codebase root path
- **F1.2** Language distribution (visual chart + file counts)
- **F1.3** Total module / file / edge counts
- **F1.4** Health score (large prominent number, 0–100, color-coded green/amber/red)
- **F1.5** Stat cards pair issue counts with their clean counterparts: `violations`, `cycles`, `gravity wells`, `red flags` on one row; **`Layers: X / N clean`** and **`Packages: X / N violation-free`** on the row alongside. Healthy state is as visible as broken state (satisfies G7).
- **F1.6** Scan timestamp + noupling version

**Acceptance criteria:**
- Header occupies ≤ 15% of the viewport vertical space at default zoom.
- All values populate from the Data Contract without any computed-in-browser logic.

### 8.3 Feature: Layered Structure Map (LSM)

The headline view. Modules arranged vertically by topological dependency depth: leaves at the bottom, entry points at the top. Read-only in v1.

- **F2.1** Topological sort: modules with no incoming dependencies sit at level 0 (top); modules they depend on flow down through levels.
- **F2.2** Each node is a Layer or module, sized by file count (or LOC — pick one, document the choice).
- **F2.3** Edges drawn between nodes; weight visualized by line thickness.
- **F2.4** Cyclic edges (those creating cycles) are colored distinctly (red) and use a different arrowhead.
- **F2.5** Rule violations are highlighted (e.g., red dashed edges).
- **F2.6** Click a node → details panel opens (F4).
- **F2.7** Double-click a node → drill down into its children (F3).
- **F2.8** Hovering a node highlights all its direct dependencies (incoming + outgoing) and dims everything else.
- **F2.9** **Layer health color.** Each Layer's background band is tinted by its violation rate: clean Layers get a cool positive hue (soft blue/green), Layers with violations warm up toward red as the violation rate climbs. Healthy structure becomes *visually present*, not merely absent of red. Palette uses the WCAG-AA palette declared in §12.5 and remains color-blind-safe. (Satisfies G7.)

**Acceptance criteria:**
- Renders correctly at 100+ nodes (typical large codebase scope).
- 60fps interaction at typical scales.
- Layout is deterministic — re-opening the same Explorer file produces an identical-looking LSM.
- Layer overlay is toggleable (button to show/hide rule-derived layer groupings).

### 8.4 Feature: Multi-Level Drill-Down

The LSM is recursive. A node representing `src/services/` can be expanded to show its sub-modules; those can be expanded further down to individual files. Breadcrumbs at the top show the current scope.

- **F3.1** Double-click drills down; a "back" button or breadcrumb segment returns up.
- **F3.2** Drill state survives view switches (matrix, force-directed in v3) within the same session.
- **F3.3** When drilled into a sub-tree, all metrics in the side panel scope to that sub-tree (count only files inside, edges only within).
- **F3.4** Breadcrumb supports clicking any segment to jump back to that level.

**Acceptance criteria:**
- Drill-down to the file level works for codebases with up to 5 levels of nesting.
- Re-rendering after drill-down is < 200ms at typical scales.

### 8.5 Feature: Node Details Panel

Clicking any node opens a panel (right side, ~30% of width) showing details for that node.

- **F4.1** Identity: full path, Layer, file count, LOC
- **F4.2** Metrics: per-file nodes show `Ca`, `Ce`, `I` (instability), LOC, blast radius upstream/downstream; per-package nodes additionally show `A` (abstractness), `D` (distance from main sequence), and `cohesion` (`null` for container packages)
- **F4.3** Incoming dependencies (clients) — list of nodes that depend on this one, with edge weights
- **F4.4** Outgoing dependencies (providers) — list of nodes this one depends on, with edge weights
- **F4.5** Cycles this node participates in (if any), with link to cycle detail
- **F4.6** Violations involving this node (if any)
- **F4.7** "Open in editor" button → constructs `vscode://file/<absolute-path>` and opens the URL.

**Acceptance criteria:**
- All metric values populate from the Data Contract; no in-browser computation in v1.
- "Open in editor" works for files. For module-level nodes, opens the directory if the editor supports it; otherwise opens the first file inside.
- Editor URL scheme is configurable (a generation-time flag `--editor vscode|jetbrains|sublime|cursor`).

### 8.6 Feature: Search and Spotting Filters

A top-bar search input that instantly filters the LSM.

- **F5.1** Substring search across node names and full paths
- **F5.2** Regex search (toggleable; tooltip indicates which mode is active)
- **F5.3** Matching nodes are highlighted; non-matching nodes are dimmed
- **F5.4** Clearing the search restores the full view
- **F5.5** "Spot" filters — toggleable pills above the LSM:
  - "Show only nodes in cycles"
  - "Show only nodes with violations"
  - "Show only clean modules" (no violations, no cycle membership, no Red Flag)
  - "Hide violations" (mute issue highlights so structure reads clearly)
  - "Show only Layer: <name>" (one chip per declared Layer)
  - "Show only nodes touched in last <N> snapshots" (uses history data if present)

**Acceptance criteria:**
- Search updates as the user types (no submit button).
- Filters compose (search + spot filter narrows to intersection).
- Highlighting is visible at all zoom levels.

### 8.7 Feature: Focus and Breadcrumb Scoping

The user can "lock" the entire Explorer view to a sub-package — all metrics, search, and visualizations scope to that container.

- **F6.1** "Focus on this node" button in the Details panel
- **F6.2** When focused, all metrics in the header recompute for the focused scope (using pre-computed sub-tree totals from the Data Contract)
- **F6.3** Breadcrumb at the top shows the focus chain (e.g., `noupling / src / reporter / explorer`)
- **F6.4** Clear focus button returns to the full codebase view

**Acceptance criteria:**
- Focusing on a module reduces visible nodes to that module's sub-tree.
- All metric values shown during focus are computed for the focused sub-tree only.

### 8.8 Feature: Layer Overlay

A toggleable overlay that visualizes the Layers declared in `.noupling/settings.json` on the LSM.

- **F7.1** Each declared Layer gets a distinct background color tint
- **F7.2** Nodes are grouped into their Layer's color region
- **F7.3** Rule arrows (allowed and forbidden directions) are rendered as legend
- **F7.4** Violations appear as red edges with a tooltip showing the rule's `message` field and a chip indicating its `source` (`dependency_rule` or `layer_order`)
- **F7.5** "Unclassified" nodes (no Layer match) get a neutral tint and a labeled warning

**Acceptance criteria:**
- Overlay toggles cleanly without re-layout.
- Rule messages display on edge hover.

### 8.9 Feature: Cycles Surface (Inline)

In v1, cycles surface inline on the LSM (red edges) and as a list in the Details panel. A separate cycle browser view comes in v3.

- **F8.1** Edges that participate in a cycle are colored red on the LSM
- **F8.2** Cycle nodes get a small badge (number of cycle members)
- **F8.3** Details panel shows cycle ID and full member list when clicked
- **F8.4** Minimum-cut suggestion (which edge to break to resolve the cycle) is shown as data in the panel; not visualized on the canvas in v1

**Acceptance criteria:**
- All cycles noupling found are surfaced.
- Cycle highlighting toggles via a single button in the toolbar.

### 8.10 Feature: Click-to-Source

Every node and edge offers navigation to the corresponding source location in the user's editor.

- **F9.1** Files: `vscode://file/<absolute-path>` (or configured editor)
- **F9.2** Modules: open the directory (where supported); fall back to the first file inside
- **F9.3** Edges: when the underlying scan recorded source lines for the dependency, link to the importing file at the right line; otherwise fall back to the importing file

**Acceptance criteria:**
- Clicking opens the editor cleanly (in editors that have the URL scheme installed).
- No-op gracefully when the editor URL scheme isn't installed (browser shows "open with" or noop; no crash, no broken state).

### 8.11 Feature: Persistent UI State

Open/closed nodes, drill state, focus state, filter settings, and panel layout persist across browser reloads of the same Explorer file (LocalStorage keyed by codebase root + scan timestamp).

- **F10.1** All toggleable UI state is restored when the page reloads
- **F10.2** "Reset view" button clears the persisted state and returns to defaults
- **F10.3** State is scoped per-file (multiple Explorer files don't trample each other's state)

**Acceptance criteria:**
- Reload preserves the user's view position, drill state, and filter selection.
- Reset restores defaults cleanly.

### 8.12 v1 — Out of scope explicitly

- Dependency Matrix view (deferred to v3)
- Force-directed cluster view (deferred to v3)
- In-memory refactoring with metric recompute (v2)
- Action plan export (v2)
- Snapshot diff / history scrubber (v3)
- Editing rules from the UI (never — see NG1)
- File watching (never — see NG3)

---

## 9. v2 — The Sandbox

### 9.1 Goal

A developer experiments with refactoring moves directly in the Explorer. Drag a module to a new parent, split a bloated module, watch metrics recompute live, iterate until the design is right, then export an action plan as a checklist. All in-memory; the source is never touched by the Explorer.

The workspace split (§5.1) is already in place from v1, so v2's WASM work does not require any further crate restructuring.

### 9.2 Architecture additions for v2

- **A1.** Add `wasm32-unknown-unknown` as a supported build target for `noupling-core` (the lib crate already exists from v1).
- **A2.** Bundle the resulting `.wasm` module (base64-inlined) into the Explorer HTML.
- **A3.** Browser-side JS layer loads the WASM, hydrates an in-memory graph from the inlined JSON data, exposes an API for virtual mutations.
- **A4.** All virtual mutations and metric recomputations run client-side in WASM.

### 9.3 Feature: Virtual Drag-and-Drop

- **F11.1** Drag any node in the LSM to another parent node to virtually relocate it
- **F11.2** Drop targets are highlighted on drag (valid containers shown in green; invalid in gray)
- **F11.3** On drop: WASM recomputes the dependency graph and emits new metrics
- **F11.4** UI updates: edges re-route, metrics in side panel refresh, header scores update, new violations appear, resolved violations disappear
- **F11.5** Undo / redo for moves (Cmd+Z / Cmd+Shift+Z)
- **F11.6** Visual diff badge on moved nodes ("Moved from <old> to <new>")

**Acceptance criteria:**
- Drag-and-drop interaction is responsive (re-render < 100ms after drop at typical codebase scale).
- Metrics recompute deterministically — undoing a move returns scores to exactly their pre-move values.
- Moves are visually distinguishable from the original layout.

### 9.4 Feature: Virtual Splitting and Merging

- **F12.1** Right-click a module → "Split…" → multi-select dialog of files to extract; specify new module name → WASM produces the split graph
- **F12.2** Right-click a module → "Merge with…" → select target module → WASM produces the merged graph
- **F12.3** Both operations are undoable
- **F12.4** Visual diff on split/merged modules

**Acceptance criteria:**
- Splits and merges recompute all metrics consistently.
- Resulting layout adapts cleanly.

### 9.5 Feature: Action Log

A persistent panel records every virtual change made in the session.

- **F13.1** Chronological list of moves, splits, merges, undos
- **F13.2** Each action entry shows source state, target state, and the metric delta caused by it
- **F13.3** Click an action to jump back to that point in the timeline (re-applies actions up to that point only)
- **F13.4** "Reset to original" clears the action log

**Acceptance criteria:**
- Action log reflects exactly the sequence of virtual changes.
- Timeline scrubbing produces graph states identical to having applied actions in order.

### 9.6 Feature: Action Plan Export

The user downloads the action log as a structured plan.

- **F14.1** "Export plan" button → browser downloads a JSON or Markdown file
- **F14.2** JSON format: schema'd structure of moves/splits/merges, intended for tooling consumption
- **F14.3** Markdown format: human-readable checklist (e.g., "1. Move `src/x.rs` from `internals` to `api`; 2. Split `src/services/` into `src/services/auth/` and `src/services/billing/`; …")
- **F14.4** Both formats include the metric deltas the plan would produce vs the current state

**Acceptance criteria:**
- Both formats round-trip: re-importing the JSON plan into a fresh Explorer reproduces the same end state.
- Markdown is editor-friendly (linkable file paths, clear action verbs).

### 9.7 Feature: Live Metric Re-Calculation

Every interactive operation triggers a recompute of all visible metrics.

- **F15.1** WASM recomputes `health_score`, all per-file `Ca`/`Ce`/`I`, all per-package `A`/`D`/`cohesion`, all violations, all cycles, on every virtual change
- **F15.2** Recompute runs in a Web Worker so the UI stays responsive
- **F15.3** Header health score shows old vs new with delta indicator (e.g., "Health 78 → 84 (+6)")
- **F15.4** "Show me only metric improvements" toggle highlights cells where the virtual state improved over the original

**Acceptance criteria:**
- Full recompute at typical codebase scale completes in < 200ms.
- Recomputed metrics match what noupling would produce if the user actually performed the moves and re-scanned (within tolerance for any non-deterministic ordering).

### 9.8 Feature: Impact Analysis Overlay

Side-by-side diff between the original codebase state and the virtual sandbox state.

- **F16.1** "Show diff" toggle splits the canvas into two LSMs (original | virtual) or overlays them
- **F16.2** Changes highlighted: green for resolved violations, red for new ones, blue for moved nodes
- **F16.3** Metric deltas summarized in a banner ("Health +6, 3 violations resolved, 1 introduced, 0 new cycles")

**Acceptance criteria:**
- Side-by-side stays in sync as the user scrolls or zooms one side.
- Per-metric delta is mathematically correct.

### 9.9 v2 — Out of scope explicitly

- Persistent server-side state (still single-user, single-machine)
- Auto-apply the action plan to source files (user does that in their editor; Explorer hands over the plan)
- Suggesting refactorings (Explorer responds to user-initiated moves; it does not propose them in v2)

---

## 10. v3 — Advanced Views

### 10.1 Goal

Add the views that aren't required for the comprehension story but offer deeper analytical capability.

### 10.2 Feature: Dependency Matrix View

A view-switch button at the top toggles between LSM and Matrix.

- **F17.1** NxN grid where each row and column is a node
- **F17.2** Cell `(i, j)` shows the edge weight from node `i` to node `j` (number of references)
- **F17.3** Cells with feedback (lower-to-upper-level references) are highlighted red
- **F17.4** Zebra-striping along the diagonal to expose cohesive clusters
- **F17.5** Clicking a cell opens the same Details panel scoped to that edge
- **F17.6** Search and Layer filters apply identically to the Matrix view
- **F17.7** Sort options: by name, by Layer, by metric (Ca, Ce, Instability)

**Acceptance criteria:**
- Renders at 200+ nodes (matrix becomes information-dense but still readable).
- Performance is acceptable at this scale (initial render < 500ms; cell hover < 16ms).

### 10.3 Feature: Force-Directed Cluster View

Another view-switch option, complementing LSM and Matrix.

- **F18.1** Nodes arranged organically by force simulation; tightly coupled nodes cluster together
- **F18.2** Cluster boundaries auto-detected (Louvain or similar clustering on top of the force layout)
- **F18.3** Layer color overlay applies the same way as on the LSM
- **F18.4** Zoom and pan
- **F18.5** Click and drag behave the same as the LSM

**Acceptance criteria:**
- Renders smoothly at 300+ nodes.
- Visualization makes coupling clusters visually obvious.

### 10.4 Feature: Cycle Browser

A dedicated view for navigating cycles found in the scan.

- **F19.1** List of all cycles in the codebase, sorted by size
- **F19.2** For each cycle: member list, minimum-cut suggestion (which edge to break), and a composition breakdown showing the exact imports that form the loop
- **F19.3** "Visualize this cycle" button — narrows the LSM to only the cycle's members
- **F19.4** "Apply minimum cut as virtual change" button (v2 integration) — pre-populates the sandbox with the suggested edge removal

**Acceptance criteria:**
- Every cycle from noupling's output is surfaced.
- Visualization narrows the canvas correctly to cycle members.

### 10.5 Feature: Snapshot History Scrubber

A timeline component along the bottom of the page that lets the user scrub through past noupling scans.

- **F20.1** Horizontal bar with markers for each snapshot in `.noupling/history.db`
- **F20.2** Dragging the scrubber loads that snapshot's data into the LSM
- **F20.3** "Compare with current" mode shows a diff between any two snapshots
- **F20.4** Metric history charts: per-node Instability over time, codebase health score over time, violation count over time

**Acceptance criteria:**
- Smooth scrubbing across snapshots (no flicker).
- Diff mode highlights nodes that were added, removed, or moved between snapshots.

### 10.6 Feature: Per-Module Fat Detection

Inline detection of bloated nodes (modules with disproportionate file count or LOC).

- **F21.1** Module-level fat: package contains more than N sub-packages or files (N configurable, default 50)
- **F21.2** File-level fat: file LOC exceeds N (default 500)
- **F21.3** Optional method-level fat if noupling extracts method-level data
- **F21.4** Fat nodes get a visual badge ("FAT") and rank higher in priority lists

**Acceptance criteria:**
- Fat thresholds are configurable at generation time (`noupling report --format explorer --fat-package 30 --fat-file 300`).
- Visual indicator distinguishes fat nodes from healthy ones at a glance.

### 10.7 Feature: Gravity Well and Red Flag Detection (Surface Existing Data)

Noupling already detects Gravity Wells (high inbound coupling) and Red Flags (Fused Sibling, Trapped Child) per its README. Surface them in the Explorer.

- **F22.1** Gravity Wells shown as nodes with a dedicated badge; size scales with inbound coupling
- **F22.2** Red Flag patterns highlighted on the canvas with their specific pattern name
- **F22.3** Tooltip explains what each pattern means and what it implies for refactoring

**Acceptance criteria:**
- All Gravity Wells and Red Flags from noupling's output are surfaced.
- Tooltip text reads as actionable guidance.

### 10.8 v3 — Out of scope explicitly

- Function-level call graphs (requires noupling to extract function-level data, which it currently does not; that's a noupling-core feature, not an Explorer feature)
- Heatmap views (Layer x Metric, Module x Metric) — defer until requested
- Export to PDF (browsers do this natively; not a v3 priority)

---

## 11. Out of Scope (All Milestones)

Repeated for emphasis. These are explicit non-features:

- **OS1.** Editing `.noupling/settings.json` from the Explorer.
- **OS2.** Re-running `noupling scan` from inside the page.
- **OS3.** File watcher / auto-refresh.
- **OS4.** Picking a folder via UI dialog inside the Explorer.
- **OS5.** Native desktop app shell (Tauri/Electron).
- **OS6.** Server-side state.
- **OS7.** Multi-user collaboration or comments.
- **OS8.** Suggesting refactorings (Explorer responds to user-initiated moves only).
- **OS9.** Function-level call graph extraction. (That's a noupling-core feature, not an Explorer feature.)
- **OS10.** MCP server. (Separate product if ever needed.)

---

## 12. Cross-Cutting Requirements

### 12.1 Performance

- v1 LSM renders at 60fps for codebases up to ~500 nodes.
- v2 metric recompute completes < 200ms at typical scales after a virtual move.
- v3 Matrix view renders at 200+ nodes within 500ms.
- Initial page load (parse HTML + inlined data + WASM where applicable) under 2s for typical codebases.

### 12.2 Compatibility

- Target browsers: latest two stable versions of Chrome, Firefox, Safari, Edge.
- `file://` loading must work (no required HTTP server).
- WASM in v2+ assumes modern browsers (no IE, no legacy Safari versions).

### 12.3 Bundle Size

- v1 single HTML file targets < 2 MB for a small codebase, < 10 MB for a 100k LOC codebase (most of which is the inlined JSON data).
- v2 adds ~500 KB–2 MB for the WASM module.
- v3 adds < 200 KB for additional view code.

### 12.4 Configuration at Generation Time

`noupling report --format explorer` accepts:

- `--editor <vscode|jetbrains|sublime|cursor|custom-url-template>` — sets the editor URL scheme
- `--output <path>` — where to write the HTML
- `--no-history` — exclude snapshot history from the bundle (smaller file)
- `--fat-package <N>` / `--fat-file <N>` — v3 fat thresholds
- `--title <string>` — customize the codebase header title

### 12.5 Accessibility

- All interactive elements reachable by keyboard
- Sufficient color contrast (WCAG AA at minimum)
- Screen-reader-friendly labels on graph nodes (aria attributes)
- Color-blind-safe palette for Layer overlays and violation highlights

### 12.6 Theming

- Light and dark themes; auto-detect via `prefers-color-scheme`
- Manual theme override via a toggle in the header

---

## 13. Open Questions

Decisions to lock down before or during v1 implementation:

- **Q1.** Visualization library for the LSM: D3.js (assumed if already used elsewhere in noupling), or custom SVG/Canvas rendering for tighter control? Decide by reviewing the existing sunburst/dashboard reporter code.
- **Q2.** Node sizing metric in the LSM: by file count, LOC, or fan-in count? Document and pick one default; consider making it user-toggleable.
- **Q3.** Click-to-source for languages outside the user's primary editor (e.g., a Rust developer with VSCode looking at a Java codebase) — does the URL scheme just open the file in the configured editor regardless, or does it require per-language editor config?

### 13.1 Resolved during drafting

- Output path: `.noupling/explorer.html` (matches the artifact convention noupling uses for its other HTML reports).
- WASM packaging (v2): base64-inlined into the HTML to preserve the single-file invariant.
- Report-format name: `explorer`.

---

## 14. Glossary (Domain Vocabulary)

Aligned with noupling's existing language.

| Term | Meaning |
|---|---|
| **Codebase** | The directory of source files noupling scanned. The thing the Explorer is about. |
| **Module** | A logical grouping of files. Maps to a noupling module — typically a directory or package. |
| **Layer** | A named, path-glob-defined group of modules from `.noupling/settings.json`. Layers express intended architecture; rules constrain dependencies between them. |
| **Rule** | A constraint on which dependencies are allowed. Two sources: explicit `dependency_rules` entries in `.noupling/settings.json` (glob-pattern `from`/`to`, `allow: bool`, `message`), and implicit layer-order rules (a Layer at index `i` may not depend on a Layer at index `j < i`). The Explorer surfaces both via the derived `effective_rules` array in the Data Contract. |
| **Violation** | A code edge that breaks a Rule. |
| **Cycle** | A strongly-connected set of files whose dependencies form a loop. Each cycle carries a minimum-cut suggestion — the edge(s) to break to make the graph acyclic. |
| **Gravity Well** | A module with disproportionately high inbound coupling — others pull toward it. |
| **Red Flag** | A structural anti-pattern (Fused Sibling, Trapped Child) detected by noupling. |
| **Fat** | A module that exceeds size or interconnectivity thresholds. |
| **Blast Radius** | The set of upstream and downstream modules affected by a change to a target. Already computed by noupling. |
| **Action Plan** | A serialized list of virtual refactoring moves a user made in the v2 sandbox. Exportable as JSON or Markdown. |
| **Snapshot** | A historical noupling scan stored in `.noupling/history.db`. The v3 scrubber navigates these. |
| **LSM (Layered Structure Map)** | The headline v1 view. Topological top-to-bottom arrangement of modules with dependency flow. |

---

## 15. Suggested Implementation Order (for the noupling agent)

For tactical sequencing when implementing v1:

1. **Workspace split.** Restructure the repo into the Cargo workspace shape from §5.1 (`crates/noupling-core/`, `crates/noupling-cli/`, empty `crates/noupling-explorer/` shell). Move existing modules with the minimum changes needed to compile. `cargo test --workspace` green before any new feature work. Update `.noupling/settings.json` with the `explorer` layer and the new `dependency_rules`; confirm `noupling audit` on its own repo passes.
2. **Reporter skeleton.** Implement `noupling-explorer`'s public entry; have `noupling-cli` register `--format explorer` and call into it. Emit a stub HTML containing only the codebase header — verify end-to-end integration before any visualization work.
3. **Data contract serialization.** Extend whatever noupling already serializes for its `--format json` to produce the Data Contract schema in section 6. Inline as `<script type="application/json">` block.
4. **LSM rendering.** Implement the layered topological layout algorithm. Render nodes and edges. Tested against the `samples/` fixtures noupling already has.
5. **Drill-down.** Expand/collapse interaction. Breadcrumb scope.
6. **Details panel + click-to-source.**
7. **Search + filters + Layer overlay.**
8. **Cycle inline surfacing.**
9. **Persistent UI state via LocalStorage.**
10. **Polish: accessibility, theming, performance pass.**

v1 ships with all of the above. v2 and v3 begin only after v1 is in active use and pain points are identified.

---

## 16. Success Criteria (How We Know It's Working)

For v1:

- A developer unfamiliar with a noupling-scanned codebase opens the Explorer, and within 60 seconds can answer: "what Layers exist, where's the highest coupling, are there cycles, where do I start reading?"
- The Explorer file is shareable as an email attachment without loss of functionality.
- A developer can drill from a top-level Layer to a single file and back without losing their place.
- Click-to-source opens the right file in the configured editor 95%+ of the time.

For v2:

- A developer experimenting with a refactor in the sandbox makes 10 virtual moves, sees `health_score` improve by ≥ 5 points, exports the action plan, and follows it to a real refactor in their editor.

For v3:

- A developer reviewing a codebase's structural history scrubs through snapshots and identifies the commit that introduced a cycle.

---

*End of PRD.*
