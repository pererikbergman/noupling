import { useEffect, useMemo, useState } from "react";
import type { DataContract, NodeEntry } from "./types";
import { TopBar } from "./components/TopBar";
import { SearchRow } from "./components/SearchRow";
import { SidePanel } from "./components/SidePanel";
import { CanvasArea } from "./components/CanvasArea";
import { DetailsPanel } from "./components/DetailsPanel";
import { ScoreDialog } from "./components/ScoreDialog";
import type { Issue } from "./components/SidePanel";
import { useExplorerState, useNodeFilter, inScope } from "./state/explorerState";
import { shortestPath, pathEdges, minCutEdges } from "./state/paths";

interface IssueFocus {
  /** Stable key matching the IssuesTab's button data-issue-key. */
  key: string;
  /** Lowest common ancestor of the participants — what scope we drilled to. */
  lca: string;
  /** Concrete edges between participants — extra-prominent on the canvas. */
  edges: Set<string>;
}

export interface AppProps {
  data: DataContract;
}

export function App({ data }: AppProps) {
  const [theme, setTheme] = useState<"dark" | "light">(
    (document.documentElement.getAttribute("data-theme") as "dark" | "light") ?? "dark",
  );
  const [scoreDialogOpen, setScoreDialogOpen] = useState(false);
  const [issueFocus, setIssueFocus] = useState<IssueFocus | null>(null);

  function toggleTheme() {
    const next = theme === "dark" ? "light" : "dark";
    document.documentElement.setAttribute("data-theme", next);
    setTheme(next);
  }

  const state = useExplorerState(data);
  const filterFn = useNodeFilter(data, state);

  const visibleData = useMemo(
    () => narrowData(data, state.scope, filterFn),
    [data, state.scope, filterFn],
  );

  // Path-finder highlight: only when the user has completed both picks.
  const pathHighlight = useMemo(() => {
    if (state.pathFinder.mode !== "done") return new Set<string>();
    const chain = shortestPath(
      data,
      state.pathFinder.from,
      state.pathFinder.to,
    );
    return pathEdges(chain);
  }, [data, state.pathFinder]);

  // Min-cut highlight: precompute from scoped cycles. Union with the
  // issue-focus edges so LSM rendering treats both as "this is the
  // edge you should look at" (#275).
  const minCutHighlight = useMemo(() => {
    const base = state.minCutShown
      ? minCutEdges(visibleData)
      : new Set<string>();
    if (issueFocus) {
      for (const e of issueFocus.edges) base.add(e);
    }
    return base;
  }, [state.minCutShown, visibleData, issueFocus]);

  function startPathFinder() {
    state.setPathFinder({ mode: "pick-from" });
  }

  // Esc clears issue focus mode (#275 acceptance — "focus mode must
  // never trap"). Spot-filter clearing also drops focus to keep state
  // coherent.
  useEffect(() => {
    if (!issueFocus) return;
    function onKey(e: KeyboardEvent) {
      if (e.key === "Escape") setIssueFocus(null);
    }
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [issueFocus]);

  function onIssueFocus(issue: Issue | null, key: string | null) {
    if (!issue || !key) {
      setIssueFocus(null);
      return;
    }
    // Collect participants per issue kind so we can compute LCA + edge
    // highlights. Falls back to single-subject when an issue family
    // doesn't enumerate multiple files.
    const participants = participantsOf(issue, data);
    const lca = longestCommonAncestor(participants);
    const edges = new Set<string>();
    if (participants.length >= 2) {
      // Highlight all directed edges between any pair of participants.
      const set = new Set(participants);
      for (const e of data.edges) {
        if (set.has(e.from) && set.has(e.to)) {
          edges.add(`${e.from}→${e.to}`);
        }
      }
    }
    if (lca !== state.scope) state.setScope(lca);
    setIssueFocus({ key, lca, edges });
  }

  function onNodePicked(id: string) {
    const pf = state.pathFinder;
    if (pf.mode === "pick-from") {
      state.setPathFinder({ mode: "pick-to", from: id });
    } else if (pf.mode === "pick-to") {
      state.setPathFinder({ mode: "done", from: pf.from, to: id });
    } else {
      // Default click → open details panel.
      state.setSelected(id);
    }
  }

  return (
    <div className="grid h-screen w-screen grid-cols-[380px_1fr] grid-rows-[auto_auto_1fr]">
      <TopBar
        data={data}
        theme={theme}
        onToggleTheme={toggleTheme}
        onResetView={state.resetView}
        viewMode={state.viewMode}
        onViewMode={state.setViewMode}
        onStartPathFinder={startPathFinder}
        pathFinderActive={state.pathFinder.mode !== "idle"}
        onShowMinCut={() => state.setMinCutShown(!state.minCutShown)}
        minCutShown={state.minCutShown}
        hasCycles={visibleData.cycles.length > 0}
      />
      <SearchRow
        data={visibleData}
        search={state.search}
        onSearch={state.setSearch}
        searchMode={state.searchMode}
        onSearchMode={state.setSearchMode}
      />
      <SidePanel
        data={data}
        scope={state.scope}
        onScope={state.setScope}
        onSelect={state.setSelected}
        onSpotFilter={(f) => {
          // Clearing spot filter (back to "all") also exits focus mode
          // — sticky-until-replaced semantics from #275.
          if (f === "all" && issueFocus) setIssueFocus(null);
          state.setSpotFilter(f);
        }}
        onScoreClick={() => setScoreDialogOpen(true)}
        foldersOnly={state.foldersOnly}
        onFoldersOnly={state.setFoldersOnly}
        activeIssueKey={issueFocus?.key ?? null}
        onIssueFocus={onIssueFocus}
      />
      <CanvasArea
        data={visibleData}
        scope={state.scope}
        codebaseTitle={codebaseTitleOf(data)}
        onScope={state.setScope}
        onClearScope={() => state.setScope("")}
        onNodeClick={onNodePicked}
        spotFilter={state.spotFilter}
        onSpotFilter={state.setSpotFilter}
        layerOverlay={state.layerOverlay}
        onToggleLayerOverlay={() => state.setLayerOverlay(!state.layerOverlay)}
        cycleHighlight={state.cycleHighlight}
        onToggleCycleHighlight={() => state.setCycleHighlight(!state.cycleHighlight)}
        viewMode={state.viewMode}
        pathFinder={state.pathFinder}
        onCancelPathFinder={() => state.setPathFinder({ mode: "idle" })}
        pathHighlight={pathHighlight}
        minCutHighlight={minCutHighlight}
        issueFocusActive={!!issueFocus}
        onCancelIssueFocus={() => setIssueFocus(null)}
      />
      <ScoreDialog
        data={data}
        open={scoreDialogOpen}
        onClose={() => setScoreDialogOpen(false)}
        onSelect={state.setSelected}
      />
      <DetailsPanel
        data={visibleData}
        selectedId={state.selected}
        onClose={() => state.setSelected(null)}
        onSelect={state.setSelected}
        onFocus={(scope) => {
          state.setScope(scope);
          state.setSelected(null);
        }}
      />
    </div>
  );
}

/**
 * Extract the participant file ids from an issue. Cycles enumerate
 * their full member list; red flags carry an explicit `modules` array
 * (passed through via the Issue.scope). Violations and gravity wells
 * carry a single subject.
 */
function participantsOf(it: Issue, data: DataContract): string[] {
  if (it.kind === "cycle") {
    const c = data.cycles.find((c) => c.members[0] === it.subject);
    return c?.members ?? [it.subject];
  }
  if (it.kind === "red-flag") {
    const f = data.red_flags.find((f) => f.modules[0] === it.subject);
    return f?.modules ?? [it.subject];
  }
  if (it.kind === "violation") {
    const v = data.violations.find(
      (v) => v.edge.from === it.subject || v.edge.to === it.subject,
    );
    return v ? [v.edge.from, v.edge.to] : [it.subject];
  }
  return [it.subject];
}

/**
 * Longest common ancestor of a set of paths. Splits each path into
 * `/`-segments and keeps prefix segments that all paths agree on.
 * Returns "" when there's nothing in common (drilling to root).
 */
function longestCommonAncestor(paths: string[]): string {
  if (paths.length === 0) return "";
  const splits = paths.map((p) => p.split("/").filter(Boolean));
  const common: string[] = [];
  for (let i = 0; ; i++) {
    const seg = splits[0][i];
    if (seg === undefined) break;
    if (!splits.every((s) => s[i] === seg)) break;
    common.push(seg);
  }
  // Drop the last segment if it's a leaf (file) — we want the
  // containing directory, not the file itself.
  if (
    common.length > 0 &&
    paths.length === 1 &&
    common.join("/") === paths[0] &&
    paths[0].includes(".")
  ) {
    common.pop();
  }
  return common.join("/");
}

function codebaseTitleOf(data: DataContract): string {
  return (
    data.report_options.title ??
    data.codebase.path.split("/").filter(Boolean).pop() ??
    data.codebase.path
  );
}

function narrowData(
  data: DataContract,
  scope: string,
  predicate: (n: NodeEntry) => boolean,
): DataContract {
  const visibleNodes = data.nodes.filter((n) => inScope(n.id, scope) && predicate(n));
  const visibleIds = new Set(visibleNodes.map((n) => n.id));
  const visibleEdges = data.edges.filter((e) => visibleIds.has(e.from) && visibleIds.has(e.to));
  const fileCount = visibleNodes.filter((n) => n.kind === "file").length;
  const moduleCount = visibleNodes.filter((n) => n.kind !== "file").length;
  const visibleCycles = data.cycles.filter((c) => c.members.every((id) => visibleIds.has(id)));
  const visibleViolations = data.violations.filter(
    (v) => visibleIds.has(v.edge.from) && visibleIds.has(v.edge.to),
  );
  return {
    ...data,
    nodes: visibleNodes,
    edges: visibleEdges,
    cycles: visibleCycles,
    violations: visibleViolations,
    summary_counts: {
      ...data.summary_counts,
      violations: visibleViolations.length,
      cycles: visibleCycles.length,
    },
    codebase: {
      ...data.codebase,
      module_count: moduleCount,
      file_count: fileCount,
      edge_count: visibleEdges.length,
    },
  };
}
