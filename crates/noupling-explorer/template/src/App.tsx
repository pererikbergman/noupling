import { useMemo, useState } from "react";
import type { DataContract } from "./types";
import { TopBar } from "./components/TopBar";
import { SearchRow } from "./components/SearchRow";
import { SidePanel } from "./components/SidePanel";
import { CanvasArea } from "./components/CanvasArea";
import { useExplorerState, inScope } from "./state/explorerState";

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

  // Derived: data viewed through the current scope. Counts shown in the
  // side panel + the search row reflect the scoped subset (PRD F3.3).
  const scopedData = useMemo(() => scopeView(data, state.scope), [data, state.scope]);

  return (
    <div className="grid h-screen w-screen grid-cols-[380px_1fr] grid-rows-[auto_auto_1fr]">
      <TopBar data={data} theme={theme} onToggleTheme={toggleTheme} />
      <SearchRow data={scopedData} />
      <SidePanel data={scopedData} />
      <CanvasArea
        data={scopedData}
        scope={state.scope}
        codebaseTitle={codebaseTitleOf(data)}
        onScope={state.setScope}
        onClearScope={() => state.setScope("")}
      />
    </div>
  );
}

function codebaseTitleOf(data: DataContract): string {
  return data.codebase.path.split("/").filter(Boolean).pop() ?? data.codebase.path;
}

/**
 * Narrow the Data Contract to the current drill scope. Files outside
 * the scope drop; edges with either endpoint outside drop too. Counts
 * are recomputed.
 *
 * The original `codebase.path`, `noupling_version`, `generated_at`,
 * `layers`, `dependency_rules`, and `effective_rules` are passed through
 * unchanged — drilling narrows *what's seen*, not the configuration.
 */
function scopeView(data: DataContract, scope: string): DataContract {
  if (scope === "") return data;
  const visibleNodes = data.nodes.filter((n) => inScope(n.id, scope));
  const visibleIds = new Set(visibleNodes.map((n) => n.id));
  const visibleEdges = data.edges.filter((e) => visibleIds.has(e.from) && visibleIds.has(e.to));
  const fileCount = visibleNodes.filter((n) => n.kind === "file").length;
  const moduleCount = visibleNodes.filter((n) => n.kind !== "file").length;
  return {
    ...data,
    nodes: visibleNodes,
    edges: visibleEdges,
    codebase: {
      ...data.codebase,
      module_count: moduleCount,
      file_count: fileCount,
      edge_count: visibleEdges.length,
    },
  };
}
