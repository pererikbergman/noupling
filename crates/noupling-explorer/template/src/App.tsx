import { useMemo, useState } from "react";
import type { DataContract, NodeEntry } from "./types";
import { TopBar } from "./components/TopBar";
import { SearchRow } from "./components/SearchRow";
import { SidePanel } from "./components/SidePanel";
import { CanvasArea } from "./components/CanvasArea";
import { DetailsPanel } from "./components/DetailsPanel";
import { useExplorerState, useNodeFilter, inScope } from "./state/explorerState";

export interface AppProps {
  data: DataContract;
}

export function App({ data }: AppProps) {
  const [theme, setTheme] = useState<"dark" | "light">(
    (document.documentElement.getAttribute("data-theme") as "dark" | "light") ?? "dark",
  );

  function toggleTheme() {
    const next = theme === "dark" ? "light" : "dark";
    document.documentElement.setAttribute("data-theme", next);
    setTheme(next);
  }

  const state = useExplorerState(data);
  const filterFn = useNodeFilter(data, state);

  // scoped+filtered Data Contract — drives every consumer downstream.
  const visibleData = useMemo(
    () => narrowData(data, state.scope, filterFn),
    [data, state.scope, filterFn],
  );

  return (
    <div className="grid h-screen w-screen grid-cols-[380px_1fr] grid-rows-[auto_auto_1fr]">
      <TopBar
        data={data}
        theme={theme}
        onToggleTheme={toggleTheme}
        onResetView={state.resetView}
      />
      <SearchRow
        data={visibleData}
        search={state.search}
        onSearch={state.setSearch}
        searchMode={state.searchMode}
        onSearchMode={state.setSearchMode}
      />
      <SidePanel data={visibleData} />
      <CanvasArea
        data={visibleData}
        scope={state.scope}
        codebaseTitle={codebaseTitleOf(data)}
        onScope={state.setScope}
        onClearScope={() => state.setScope("")}
        onNodeClick={state.setSelected}
        spotFilter={state.spotFilter}
        onSpotFilter={state.setSpotFilter}
        layerOverlay={state.layerOverlay}
        onToggleLayerOverlay={() => state.setLayerOverlay(!state.layerOverlay)}
        cycleHighlight={state.cycleHighlight}
        onToggleCycleHighlight={() => state.setCycleHighlight(!state.cycleHighlight)}
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

/**
 * Narrow the Data Contract to the active scope ∩ active filter set.
 * Both edges drop when either endpoint drops. Counts recompute so the
 * side panel + search row reflect the visible subset.
 */
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
