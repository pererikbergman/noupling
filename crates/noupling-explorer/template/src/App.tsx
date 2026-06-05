import { useMemo, useState } from "react";
import type { DataContract, NodeEntry } from "./types";
import { TopBar } from "./components/TopBar";
import { SearchRow } from "./components/SearchRow";
import { SidePanel } from "./components/SidePanel";
import { CanvasArea } from "./components/CanvasArea";
import { DetailsPanel } from "./components/DetailsPanel";
import { ScoreDialog } from "./components/ScoreDialog";
import { useExplorerState, useNodeFilter, inScope } from "./state/explorerState";
import { shortestPath, pathEdges, minCutEdges } from "./state/paths";

export interface AppProps {
  data: DataContract;
}

export function App({ data }: AppProps) {
  const [theme, setTheme] = useState<"dark" | "light">(
    (document.documentElement.getAttribute("data-theme") as "dark" | "light") ?? "dark",
  );
  const [scoreDialogOpen, setScoreDialogOpen] = useState(false);

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

  // Min-cut highlight: precompute from scoped cycles.
  const minCutHighlight = useMemo(
    () => (state.minCutShown ? minCutEdges(visibleData) : new Set<string>()),
    [state.minCutShown, visibleData],
  );

  function startPathFinder() {
    state.setPathFinder({ mode: "pick-from" });
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
        onSpotFilter={state.setSpotFilter}
        onScoreClick={() => setScoreDialogOpen(true)}
        foldersOnly={state.foldersOnly}
        onFoldersOnly={state.setFoldersOnly}
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
