# noupling Explorer — template

Frontend subproject that produces the single-file HTML the `noupling-explorer`
Rust crate embeds and emits when a user runs `noupling report --format explorer`.

**This is its own project with its own toolchain (pnpm + Vite + React + Tailwind).**
The noupling Rust workspace never invokes pnpm; cargo treats this directory as
opaque. The committed `dist/explorer.html` is what the Rust crate actually
consumes via `include_str!`, and CI drift-checks it.

See `docs/noupling-explorer-prd.md` §5.2 + §5.2.1 for the architecture; see
`docs/noupling-explorer-design.md` for the visual brief.

## Quick start

```
cd crates/noupling-explorer/template
pnpm install
pnpm dev                  # hot-reload dev server against ./public/samples/acme-payments.json
pnpm build                # produce dist/explorer.html (single self-contained file)
pnpm test:e2e             # Playwright smoke suite (boots dev server, drives sample)
pnpm test:e2e:android     # manual regression suite against a real codebase
```

Open `http://localhost:5174/?sample=<name>` during dev to swap samples.

### Real-codebase regression suite

The `tests/e2e/android.spec.ts` suite exercises the parts of the
Explorer that historically broke on real-world projects (URL-slash
bugs, empty LSM on deeply-nested codebases, auto-layer banner, score
rendering). It's gated on `NOUPLING_E2E_ANDROID_REPO` and does *not*
run in CI — point it at any noupling-scanned repo locally:

```
noupling-dev report ~/my/project --format explorer --editor vscode
NOUPLING_E2E_ANDROID_REPO=~/my/project pnpm test:e2e:android
```

The smoke suite (`pnpm test:e2e`) does run in CI on every PR.

## Data Contract

The frontend reads its data from one of two places:

1. **Production** (Rust-emitted HTML opened via `file://`): the
   `<script id="noupling-data" type="application/json">` element in
   `index.html` is filled at report-emission time by the Rust crate's
   `noupling_explorer::render(...)`. The TypeScript shape is in
   `src/types.ts`; the Rust serializer is in
   `crates/noupling-explorer/src/data_contract.rs`. The two are
   version-locked — bump both together.

2. **Dev** (`pnpm dev`): the script tag is empty, so `src/data.ts`
   falls back to fetching `/samples/<name>.json` (default
   `acme-payments`). Add more JSON files under `public/samples/` to
   iterate against different shapes — gravity-well-heavy, cycle-heavy,
   monorepo, etc.

The injection contract is intentionally minimal: one `<script>` tag, one
`id="root"` element. The template is free to evolve its layout, styling,
and components without breaking Rust.

## Rebuild-before-commit rule

`dist/explorer.html` is checked into git because the Rust crate embeds it
at compile time and cargo must not depend on npm. Whenever you change
anything under `src/`, `index.html`, or any config, **rebuild and commit
both source and dist in the same PR**:

```
pnpm build
git add src dist
git commit -m "..."
```

A drift-check CI job runs `pnpm install && pnpm build` on every PR and
fails if `dist/explorer.html` differs from what the source produces.

## Layout reference

The current shell implements **Variant D** from the prototype
(`prototype/explorer-ui-mockup` branch):

- Top bar with view-mode pills (LSM / Matrix / Composition), an
  Inside / + External scope toggle, hide-by-kind chips, and Structure101-
  style utility icons (path finder, cycle min-cut, filter, export).
- Search row with substring/regex modes.
- Left-side tabbed panel (Info / Files / Rules / Plan): Info carries the
  Codebase Header + Steps + Stats; Files lists the node tree; Rules
  surfaces `effective_rules` with the source chip; Plan is the v2
  Sandbox placeholder.
- Canvas area on the right with overlay filter pills, zoom controls
  bottom-left, action-plan strip bottom-right. The actual Layered
  Structure Map renders here in slice **#233**.
- Light/dark theme via `prefers-color-scheme` auto-detect + a manual
  toggle in the top bar.

## File map

```
public/samples/         JSON fixtures for `pnpm dev`
src/
├── App.tsx             Grid layout (top bar / search row / panel / canvas)
├── main.tsx            Bootstraps React with the loaded Data Contract
├── data.ts             Reads <script id="noupling-data"> or falls back to /samples/
├── types.ts            TypeScript mirror of the Rust DataContract
├── sourceLink.ts       Editor URL templates (vscode/jetbrains/sublime/cursor)
├── styles.css          Tailwind base + design tokens (dark + light)
├── components/
│   ├── TopBar.tsx
│   ├── SearchRow.tsx
│   ├── SidePanel.tsx   Tabs: Info / Files / Rules / Plan
│   ├── Breadcrumb.tsx  Drill chain shown above the canvas
│   ├── DetailsPanel.tsx Right-slide panel on node click
│   └── CanvasArea.tsx  Hosts the LSM + filter pills + toolbar
├── lsm/
│   ├── LSM.tsx         SVG renderer (cards, edges, layer bands, badges)
│   └── layout.ts       Pure Data Contract → SVG geometry
└── state/
    └── explorerState.ts  Single owner; LocalStorage-persisted UI state.
dist/explorer.html      Build artifact — consumed by the Rust crate.
                        Must be regenerated whenever src/ changes.
```

## v1 is complete (#231–#242)

What ships in v1:
- **Codebase header** (#231) — language distribution, module/file/edge counts, health score, stat cards, scan timestamp + noupling version.
- **Layered Structure Map** (#233) — file nodes in tiers by layer, edges weighted, layer bands tinted by violation rate.
- **Drill-down + breadcrumb** (#234) — double-click to scope; breadcrumb above canvas, click any segment to jump.
- **Details panel + click-to-source** (#235) — single-click → right-side panel; "Open in editor" supports vscode/jetbrains/sublime/cursor + custom URL template.
- **Focus** (#236) — "Focus on this node" button in the details panel locks the view to a sub-tree.
- **Search** (#237) — substring + regex live filtering.
- **Spot filters** (#238) — All / In cycles / With violations / Clean / Hide violations / Gravity wells.
- **Layer overlay** (#239) — "L" toolbar button paints per-layer-name tints.
- **Cycles inline** (#240) — red edges + per-node cycle-count badge + toolbar toggle.
- **Persistence** (#241) — every UI toggle persists per Explorer file. ↺ Reset View clears.
- **Polish** (#242) — keyboard navigation (Tab to focus, Enter to select, D to drill), ARIA labels on icon buttons, visible focus ring, prefers-color-scheme auto theme.

## Next slices
- **#235** — node click → details in the Info tab (selection mode).
- **#236–#242** — focus, search, filters, layer overlay, cycles, persistence, polish.
