# Explorer Bug Reports / Polish Backlog

Collected during manual review of the Explorer report. Numbered for later conversion to GitHub issues.

> Each bug also has its own file under `docs/bugs/` with an **Open questions** section for clarifications. The per-bug files are the working copies; this file is the index.

---

## 1. Info tab: health score hidden below the fold by Auto-Layers banner

**Area:** Explorer — left pane, Info tab (`SidePanel.tsx` → `InfoTab`)

**Current order:** WelcomeCard → AutoLayersBanner → Stats (contains Health score) → History.

**Problem:** When layers are auto-detected, the `AutoLayersBanner` (with its explanation and copy-to-clipboard settings snippet) takes up enough vertical space that the Health score in the Stats section is pushed below the fold. You have to scroll to see it — but the health score is the headline number of the entire tool.

**Expected:** Health score visible without scrolling, above the Auto-Layers banner.

**Suggested fix:** Reorder `InfoTab` so Stats (or at minimum the Health score row) renders before the `AutoLayersBanner`, e.g.: WelcomeCard → Stats → History → AutoLayersBanner. The banner is contextual/one-time information and can live further down.

**Severity:** Medium (UX, first-impression of the report)

---

## 2. Files tab: no way to navigate back up + needs a folders-only filter

**Area:** Explorer — left pane, Files tab (`SidePanel.tsx` → `FilesTab` / `TreeRow`)

**Problem (a) — no "up" affordance:** Clicking a folder row calls `onScope(node.id)` and drills the whole tab down into that folder. Once drilled, the tab itself offers no way back up — the only breadcrumb lives in `CanvasArea`, not in the side panel. Backing out of a deep scope is needlessly hard.

**Problem (b) — files clutter the tree:** The tree always shows files mixed in with folders. When you're trying to understand structure, files are noise.

**Expected:**
- The Files tab behaves as a traditional file tree (expand/collapse, which it already does), with an easy, always-visible way to go up one level / back to root from inside the tab (e.g. an up-row "..", or its own breadcrumb).
- A filter toggle in the tab (e.g. "Hide files" / "Folders only") that hides `kind === "file"` nodes so only directories/packages are shown.

**Severity:** Medium (navigation dead-end)

---

## 3. New fifth tab "Layers": Finder-style one-level-at-a-time drill-down

**Area:** Explorer — left pane (`SidePanel.tsx`, tab strip currently `Info | Files | Issues | Rules`)

**Request:** Add a fifth tab, **Layers**, complementing the Files tree. Inspired by Structure101 / macOS Finder navigation:

- Shows **only the immediate children** of the current folder/scope — one level at a time, no nested tree.
- Double-click (or click) a child to dig down into it.
- A very easy, prominent way to go **up** again — this is the critical part; same weakness as bug #2. E.g. an always-visible up button + breadcrumb at the top of the tab.
- Each child should presumably carry its layer color/accent and key metrics, as in the current rows.

**Rationale:** The current Files tree and the canvas both try to show depth at once. A leveled view ("what are the parts at *this* level?") matches how noupling itself reasons (BFS per directory level) and how Structure101 presents architecture.

**Note:** Keep Files as the traditional tree (bug #2); Layers is the leveled drill-down. Both navigation styles are wanted.

**Severity:** Enhancement

---

## 4. Issues tab: hover highlight nearly black + click gives no visible feedback

**Area:** Explorer — left pane, Issues tab (`SidePanel.tsx` → `IssuesTab`); canvas (`CanvasArea` / LSM)

**Problem (a) — highlight color:** Issue cards use `hover:bg-pill` which resolves to `--pill-active-bg` (28–42 RGB, i.e. near-black). Hovering an issue card makes it almost black — far too heavy. It should just be slightly darker than the card background, enough to show which card you're on.

**Problem (b) — click appears to do nothing:** Clicking an issue calls `onSpotFilter` + `onSelect`/`onScope`, but the resulting change on the canvas is not perceivable. No visible reaction = feels broken.

**Expected on click:** The violation is shown *visually* on the canvas:

- For a cycle / coupling violation between two modules: render each module as a large container box, with its classes/files grouped inside, and highlight the specific offending files and the dependency edges between them (which class in A depends on which class in B).
- Generalize: clicking any issue focuses the canvas on the participants and highlights exactly the edges/files that constitute the issue.

**Suggested direction:** A dedicated "issue focus" canvas mode — scope to the lowest common parent of the participants, expand participants to file level (grouped by module container), color the offending edges, dim everything else. Plus an obvious selected state on the clicked card (also fixes (a)'s missing selection feedback — currently only hover styling exists, no selected styling at all).

**Severity:** High (core value of the Issues tab — #265 shipped the list, but the list → visual loop is broken)

---

## 5. Issue detail view: no legend / no motivation for the classification

**Area:** Explorer — Issues tab detail view / highlighting

**Problem:** When opening an issue's detail view, nothing explains *why* the item received its status (HIGH VIOLATION, RED FLAG, CYCLE, GRAVITY WELL) or why it is bad. The reasoning exists in the analyzer (RRI = direction weight × density, red-flag definitions, gravity-well 2×-median rule) but is invisible in the UI. Today the only explanation is the `title` hover tooltip on the card — undiscoverable.

**Expected:** A legend/explanation in the detail view stating: what this issue kind means, why this instance was flagged (the numbers that triggered it: direction, density, RRI, threshold), and why it matters architecturally. The Metrics Guide content from the HTML report could be reused per issue kind.

**Severity:** Medium (comprehension — users can't act on a verdict they don't understand)

---

## 6. Issues tab: cycle items show the wrong (or arbitrary) edge on the second row

**Area:** Explorer — Issues tab (`SidePanel.tsx` → `buildIssues`, cycle items)

**Problem:** For a cycle, the second row (subtitle) shows the member path, e.g. `data → utils → data`. The first edge shown may be the *legitimate* direction — "data relies on utils" is normally fine. The actually-wrong edge is the other side of the cycle. This misleads: the row appears to accuse a healthy dependency.

**Expected:** The second row should show the edge noupling assumes is wrong — per the existing rule, the direction with the fewest dependencies in the cycle (the weakest link / minimum cut, which the analyzer already computes: `c.minimum_cut`). E.g. `break: utils → data (2 deps, vs 14 the other way)`. The full cycle path can stay in the detail view.

**Note:** `buildIssues` already has `minimum_cut` in scope — it's used in the tooltip `description` ("break X → Y to resolve") but not surfaced in the visible subtitle. Mostly a re-plumbing of existing data.

**Severity:** Medium (misleading information, low fix cost)

---

## 7. Rules tab: virtual (local-only) rules — what-if exploration

**Area:** Explorer — left pane, Rules tab

**Status:** The tab itself is fine as-is.

**Request:** Allow adding a *virtual rule* that applies only locally inside the current Explorer view — in-memory, never written to `.noupling/settings.json` (consistent with PRD NG1/NG8). The user could sketch "what if this dependency direction were forbidden?" and see which edges would light up as violations.

**Caveat:** Violation evaluation currently happens in Rust at generation time; a purely client-side rule engine would mean re-implementing rule matching in the web view, or shipping precomputed edge data rich enough to evaluate globs in JS. Possibly large.

**Priority: VERY LOW.** Explicitly deprioritized — do not pick up before items 1–6. Aligns with PRD G6 (v2 what-if exploration), so park it there.

---

## 8. Top bar: Force view not implemented

**Area:** Explorer — top navigation (`TopBar.tsx`), canvas

**Current state:** The view switcher shows LSM / Matrix / Force / Composition, but Force is a `DisabledViewBtn` ("ships in v3, PRD §10.3"). `ViewMode` in `explorerState.ts` only knows `"lsm" | "matrix"`.

**Expected (per PRD §10.3):** Force-directed cluster view — nodes arranged by force simulation so tightly coupled nodes cluster; auto-detected cluster boundaries (Louvain or similar); same layer color overlay, zoom/pan, click/drag as LSM. Renders smoothly at 300+ nodes.

**Note:** Purely algorithmic — all required data (nodes, edges) is already in the data contract. No LLM enrichment needed.

**Severity:** Enhancement (v3 feature, now wanted)

---

## 9. Top bar: Composition view not implemented (and underspecified)

**Area:** Explorer — top navigation (`TopBar.tsx`), canvas

**Current state:** Composition is a `DisabledViewBtn` whose tooltip literally says "PRD §10.x" — the view has no pinned PRD section. The closest thing in the PRD is the Cycle Browser's "composition breakdown" (§10.4 F19.2).

**Needed first:** A definition. What does the Composition view show? Candidate: a "what is this codebase made of" view — modules as containers with their contents, possibly with natural-language descriptions of each module's purpose/responsibility.

**Dependency:** If the view includes module purpose descriptions or other semantic information that can't be derived from imports alone, that data must come from LLM enrichment — see #10. The view should render gracefully without it (empty/derived-only state) and richer with it.

**Severity:** Enhancement (needs a mini-spec before implementation)

---

## 10. LLM enrichment: data-contract slots + user-runnable skills

**Area:** noupling-explorer Rust (`data_contract.rs`, render pipeline) + new skills shipped with the project

**Request:** For any Explorer data that requires an LLM to produce (module purpose summaries, architecture narratives, refactoring rationale — e.g. for the Composition view, #9):

1. **Reserve slots in the dataset.** Add optional LLM-enrichment fields to the data contract (e.g. `llm` block per node and per codebase: summary, responsibility, tags, generated_at, model). The contract doc already states fields are additive and the template treats unknown keys as optional — so this is contract-compatible.
2. **Persist enrichment outside the generated report.** The data contract is rebuilt on every `noupling report` run, so LLM output must live in a sidecar file (e.g. `.noupling/enrichment.json`) that the Rust renderer merges into the contract at generation time. Stale-detection (file hash / snapshot id per entry) so outdated summaries are flagged or dropped.
3. **Ship skills users can apply to their own project.** Claude skills (distributed with noupling) that the user runs against their codebase; the skill reads the scan output, generates the enrichment, and writes/updates the sidecar file. Next `noupling report --format explorer` picks it up.

**Open questions:** Sidecar schema versioning; should enrichment be committed to the user's repo (probably yes — it's slow to regenerate); per-module vs. per-directory granularity.

**Severity:** Enhancement (infrastructure; prerequisite for the LLM-dependent parts of #9)

