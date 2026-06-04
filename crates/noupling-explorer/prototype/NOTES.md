# Explorer UI prototype — notes

**Question:** What should the Explorer's main view look like?

**Status:** Decided — **Variant D** (with side panel on the LEFT).

## Decision

**Winner:** Variant D — Canvas-dominant layout with a left-side tabbed panel
(Info / Files / Rules / Plan), Structure101-style functional controls in the
top bar, and an overlay action-plan strip at the bottom of the canvas.

**References pulled from:**
- **Layout** — [understand-anything.com/demo](https://understand-anything.com/demo/):
  thick top bar with view-mode pills + functional toggles + utility icons,
  prominent search row, dominant canvas, tabbed side panel with a guided
  tour, overlay zoom controls inside the canvas.
- **Functional model** — **Structure101**: LSM / Matrix / Force / Composition
  view modes; Inside / + External scope toggle; "Find dependency path between
  two nodes"; min-cut suggestion for cycles; hide-by-kind chips; action plan
  with projected health delta.

**Deviation from the reference:** the side panel sits on the **left**, not
the right. The LSM canvas reads top-to-bottom (UI → domain → infra), so
reading flow + selection chrome on the left, work surface on the right,
matches the natural eye path. Mirrors editor sidebars (VSCode, Linear) the
target persona already lives in daily.

**Why this beats A/B/C:**
- A (canvas-first minimal) was too quiet about violations/cycles/health for
  onboarding personas (PRD §3.1, §3.2).
- B (three-pane workspace) had too many stat cards at the top and a left
  tree that duplicates information available in the panel.
- C (sidebar tree navigation) put the wrong things in the rail (the file
  tree is in noupling's data, not the user's daily navigation surface).
- D leans on the side panel for *guided learning* — the "Steps" list is
  the onboarding mechanic from PRD §3.1, made first-class.

## Loser variants — what to steal

- **From A** — the "details panel slides in on click" interaction. The
  current Info tab is great for onboarding; selection details should appear
  inside the same tab when a node is clicked (the "details panel" of PRD
  §8.5 lives in the Info tab body, not as a separate overlay).
- **From B** — the always-visible *hotspot* mini-cards in the side panel.
  Surface 2–3 hotspots near the top of the Info tab when no node is
  selected, so the panel never reads as empty.
- **From C** — the breadcrumb at the top of the canvas (above the LSM) and
  the view-mode toggles next to it. D currently puts view modes in the top
  bar; keeping them there + adding breadcrumb on the canvas works.

## What's still TBD inside Variant D

- Right-side action-plan strip vs Plan tab — should the queued-actions
  summary live as an overlay or only in the Plan tab? Currently both.
- File tree (Files tab) shape: tree vs flat list. Defer to #232.
- Rules tab content — likely a table of the `effective_rules` block from
  the Data Contract with violation counts; defer to the layer-overlay slice
  (#239).

## Folding forward

Variant D is the seed for **#232 — Template subproject bootstrap**. When
that issue starts:

1. Bootstrap `crates/noupling-explorer/template/` with the chosen stack
   (Vite + shadcn/ui per the visual brief).
2. Reproduce the Variant D layout as the initial app shell: top bar →
   search row → grid of (side panel | canvas-area).
3. Wire the side panel's Info tab to the Data Contract's codebase header
   + a static "Steps" placeholder (real onboarding tour is later).
4. Canvas placeholder for #233 to fill with the real LSM.

After #232 lands, delete this `prototype/` directory and its NOTES.

## How to view (until deletion)

```
open crates/noupling-explorer/prototype/explorer.html
```

The page defaults to Variant D. Use the floating bottom switcher or `←` /
`→` arrow keys to peek at the losing variants for comparison while #232
is being built.
