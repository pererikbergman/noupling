import { useEffect, useMemo, useState } from "react";
import type { DataContract, NodeEntry } from "./shared/types";
import { TopBar } from "./components/TopBar";
import { SearchRow } from "./components/SearchRow";
import { SidePanel } from "./components/SidePanel";
import { CanvasArea } from "./components/CanvasArea";
import { DetailsPanel } from "./components/DetailsPanel";
import { EdgeDetailsPanel } from "./components/EdgeDetailsPanel";
import { ScoreDialog } from "./components/ScoreDialog";
import type { Issue } from "./components/SidePanel";
import {
  useExplorerStore,
  homeScope,
  clampScope,
  useNodeFilter,
  inScope,
  shouldHighlightViolations,
} from "./state/explorerState";
import { shortestPath, pathEdges, minCutEdges } from "./state/paths";
import { computeIssueFocus, type IssueFocus } from "./state/issueFocus";
import { buildHighlightPolicy } from "./state/highlightPolicy";
import { cycleMembershipCounts } from "./state/queries";

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

  const store = useExplorerStore(data);
  // The stored scope "" means "home": the first level of the tree that
  // branches (#397). Every consumer sees the resolved scope, and a
  // request to go above home lands on home.
  const home = useMemo(() => homeScope(data), [data]);
  const state = useMemo(() => {
    const scope = clampScope(store.scope, home);
    const setState: typeof store.setState = (patch) =>
      store.setState(
        patch.scope === undefined ? patch : { ...patch, scope: clampScope(patch.scope, home) },
      );
    return { ...store, scope, setState };
  }, [store, home]);
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

  // Bundle every highlight-related input into one policy so CanvasArea
  // and the LSM stop carrying eight separately-threaded props each.
  // #318 — the policy owns the precedence rule (selected > path >
  // minCut > violation > cycle > default); EdgePath just asks it.
  const highlight = useMemo(
    () =>
      buildHighlightPolicy({
        pathEdges: pathHighlight,
        minCutEdges: minCutHighlight,
        cyclesByNode: cycleMembershipCounts(visibleData),
        selectedEdge: state.selectedEdge,
        highlightViolations: shouldHighlightViolations(state.spotFilter),
        highlightCycles: state.cycleHighlight,
        layerOverlay: state.layerOverlay,
        issueFocus,
      }),
    [
      pathHighlight,
      minCutHighlight,
      visibleData,
      state.selectedEdge,
      state.spotFilter,
      state.cycleHighlight,
      state.layerOverlay,
      issueFocus,
    ],
  );

  function startPathFinder() {
    state.setState({ pathFinder: { mode: "pick-from" } });
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
    const focus = computeIssueFocus(issue, key, data);
    // Drop any lingering edge selection: the EdgeDetailsPanel from an
    // earlier edge click is unrelated to the issue being focused and
    // would otherwise sit under the Issues list for the whole session.
    state.setState({ scope: focus.lca, selectedEdge: null });
    setIssueFocus(focus);
  }

  function onNodePicked(id: string) {
    const pf = state.pathFinder;
    if (pf.mode === "pick-from") {
      state.setState({ pathFinder: { mode: "pick-to", from: id } });
    } else if (pf.mode === "pick-to") {
      state.setState({ pathFinder: { mode: "done", from: pf.from, to: id } });
    } else {
      // Default click → open details panel.
      state.setState({ selected: id });
    }
  }

  return (
    <div
      className={
        "grid h-screen w-screen grid-rows-[auto_auto_1fr] " +
        // Right details column expands inline only when a node or an
        // edge is selected — otherwise the canvas keeps all the room.
        (state.selected || state.selectedEdge
          ? "grid-cols-[380px_1fr_360px]"
          : "grid-cols-[380px_1fr]")
      }
    >
      <TopBar
        data={data}
        theme={theme}
        onToggleTheme={toggleTheme}
        onResetView={state.reset}
        viewMode={state.viewMode}
        onViewMode={(viewMode) => state.setState({ viewMode })}
        onStartPathFinder={startPathFinder}
        pathFinderActive={state.pathFinder.mode !== "idle"}
        onShowMinCut={() => state.setState({ minCutShown: !state.minCutShown })}
        minCutShown={state.minCutShown}
        hasCycles={visibleData.cycles.length > 0}
      />
      <SearchRow
        data={visibleData}
        search={state.search}
        onSearch={(search) => state.setState({ search })}
        searchMode={state.searchMode}
        onSearchMode={(searchMode) => state.setState({ searchMode })}
      />
      <SidePanel
        data={data}
        scope={state.scope}
        homeScope={home}
        onScope={(scope) => {
          if (issueFocus && scope !== issueFocus.lca) setIssueFocus(null);
          state.setState({ scope });
        }}
        onSelect={(selected) => state.setState({ selected })}
        onSpotFilter={(spotFilter) => {
          // Clearing spot filter (back to "all") also exits focus mode
          // — sticky-until-replaced semantics from #275.
          if (spotFilter === "all" && issueFocus) setIssueFocus(null);
          state.setState({ spotFilter });
        }}
        onScoreClick={() => setScoreDialogOpen(true)}
        foldersOnly={state.foldersOnly}
        onFoldersOnly={(foldersOnly) => state.setState({ foldersOnly })}
        activeIssueKey={issueFocus?.key ?? null}
        onIssueFocus={onIssueFocus}
      />
      <CanvasArea
        data={visibleData}
        scope={state.scope}
        codebaseTitle={codebaseTitleOf(data)}
        onScope={(scope) => {
          // Leaving the participants' shared parent ends the focus; the
          // banner must never describe a canvas it no longer applies to.
          if (issueFocus && scope !== issueFocus.lca) setIssueFocus(null);
          state.setState({ scope });
        }}
        onClearScope={() => {
          setIssueFocus(null);
          state.setState({ scope: "" });
        }}
        onNodeClick={onNodePicked}
        spotFilter={state.spotFilter}
        onSpotFilter={(spotFilter) => state.setState({ spotFilter })}
        onToggleLayerOverlay={() =>
          state.setState({ layerOverlay: !state.layerOverlay })
        }
        onToggleCycleHighlight={() =>
          state.setState({ cycleHighlight: !state.cycleHighlight })
        }
        viewMode={state.viewMode}
        pathFinder={state.pathFinder}
        onCancelPathFinder={() => state.setState({ pathFinder: { mode: "idle" } })}
        onCancelIssueFocus={() => setIssueFocus(null)}
        highlight={highlight}
        onEdgeClick={(from, to) =>
          state.setState({ selectedEdge: { from, to }, selected: null })
        }
      />
      <ScoreDialog
        data={data}
        open={scoreDialogOpen}
        onClose={() => setScoreDialogOpen(false)}
        onSelect={(selected) => state.setState({ selected })}
      />
      <DetailsPanel
        data={visibleData}
        selectedId={state.selected}
        onClose={() => state.setState({ selected: null })}
        onSelect={(selected) =>
          state.setState({ selected, selectedEdge: null })
        }
        onFocus={(scope) => state.setState({ scope, selected: null })}
      />
      <EdgeDetailsPanel
        data={data}
        selectedEdge={state.selectedEdge}
        onClose={() => state.setState({ selectedEdge: null })}
        onSelectNode={(selected) =>
          state.setState({ selected, selectedEdge: null })
        }
        onScope={(scope) => state.setState({ scope, selectedEdge: null })}
      />
    </div>
  );
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
  // Edges are scope-filtered but NOT predicate-filtered: file-level
  // edges must survive even when the spot filter narrows visible
  // nodes to packages, because the canvas aggregation maps each file
  // edge up to its visible-ancestor pair (the file paths themselves
  // are deliberately absent from `visibleIds` in that case). Previously
  // we required both endpoints in `visibleIds`, which silently emptied
  // the Matrix view whenever a non-file-level filter was active.
  const allScopeIds = new Set(
    data.nodes.filter((n) => inScope(n.id, scope)).map((n) => n.id),
  );
  const visibleEdges = data.edges.filter(
    (e) => allScopeIds.has(e.from) && allScopeIds.has(e.to),
  );
  // `module_count` keeps the contract's meaning (core calls every file a
  // module); directory counts are derived from `nodes` where shown (#402).
  const fileCount = visibleNodes.filter((n) => n.kind === "file").length;
  const moduleCount = fileCount;
  const visibleCycles = data.cycles.filter((c) => c.members.every((id) => allScopeIds.has(id)));
  const visibleViolations = data.violations.filter(
    (v) => allScopeIds.has(v.edge.from) && allScopeIds.has(v.edge.to),
  );
  // An Issue is in scope when at least one participant is: a directory-
  // shaped Issue about `src/bag` stays visible while drilled into it.
  const inScopeId = (id: string) => allScopeIds.has(id) || inScope(id, scope);
  const visibleIssues = data.issues.filter((i) => i.participants.some(inScopeId));
  return {
    ...data,
    nodes: visibleNodes,
    edges: visibleEdges,
    cycles: visibleCycles,
    violations: visibleViolations,
    issues: visibleIssues,
    summary_counts: {
      ...data.summary_counts,
      violations: visibleViolations.length,
      cycles: visibleCycles.length,
      issues: visibleIssues.length,
      new_issues: visibleIssues.filter((i) => !i.baselined).length,
      baselined_issues: visibleIssues.filter((i) => i.baselined).length,
      by_kind: data.summary_counts.by_kind.map((k) => ({
        ...k,
        count: visibleIssues.filter((i) => i.kind === k.kind).length,
      })),
    },
    codebase: {
      ...data.codebase,
      module_count: moduleCount,
      file_count: fileCount,
      edge_count: visibleEdges.length,
    },
  };
}
