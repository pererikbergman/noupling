import { useEffect, useMemo, useState } from "react";
import type { DataContract, NodeEntry } from "./types";
import { TopBar } from "./components/TopBar";
import { SearchRow } from "./components/SearchRow";
import { SidePanel } from "./components/SidePanel";
import { CanvasArea } from "./components/CanvasArea";
import { DetailsPanel } from "./components/DetailsPanel";
import { EdgeDetailsPanel } from "./components/EdgeDetailsPanel";
import { ScoreDialog } from "./components/ScoreDialog";
import type { Issue } from "./components/SidePanel";
import { useExplorerStore, useNodeFilter, inScope } from "./state/explorerState";
import { shortestPath, pathEdges, minCutEdges } from "./state/paths";

interface IssueFocus {
  /** Stable key matching the IssuesTab's button data-issue-key. */
  key: string;
  /** Lowest common ancestor of the participants — what scope we drilled to. */
  lca: string;
  /** Concrete edges between participants — extra-prominent on the canvas. */
  edges: Set<string>;
  /** Immediate-children-of-LCA containers that hold at least one
   *  participant. These render *expanded* on the canvas (file children
   *  shown inline) instead of as a single container card. #275
   *  follow-up. */
  expandedContainers: Set<string>;
  /** The actual participant file ids that live under those containers
   *  — the only files to surface in the expanded render so the canvas
   *  doesn't hairball on many-file packages. */
  participantFiles: Set<string>;
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

  const state = useExplorerStore(data);
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
    // Determine which *immediate child of LCA* each participant lives
    // under — those are the containers we'll expand inline on the
    // canvas so the offending edges render at file level. Participants
    // that ARE the immediate child stay as themselves.
    const expandedContainers = new Set<string>();
    const participantFiles = new Set<string>();
    const lcaPrefix = lca === "" ? "" : lca + "/";
    for (const p of participants) {
      if (!p.startsWith(lcaPrefix) && p !== lca) continue;
      const rest = p === lca ? "" : p.slice(lcaPrefix.length);
      const firstSegment = rest.split("/")[0];
      const container = lca === "" ? firstSegment : `${lca}/${firstSegment}`;
      expandedContainers.add(container);
      // Only add as a participant file if it's actually a file node
      // (an edge endpoint may be a package path on some samples).
      const node = data.nodes.find((n) => n.id === p);
      if (node?.kind === "file") participantFiles.add(p);
    }
    if (lca !== state.scope) state.setState({ scope: lca });
    setIssueFocus({
      key,
      lca,
      edges,
      expandedContainers,
      participantFiles,
    });
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
        onScope={(scope) => state.setState({ scope })}
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
        onScope={(scope) => state.setState({ scope })}
        onClearScope={() => state.setState({ scope: "" })}
        onNodeClick={onNodePicked}
        spotFilter={state.spotFilter}
        onSpotFilter={(spotFilter) => state.setState({ spotFilter })}
        layerOverlay={state.layerOverlay}
        onToggleLayerOverlay={() =>
          state.setState({ layerOverlay: !state.layerOverlay })
        }
        cycleHighlight={state.cycleHighlight}
        onToggleCycleHighlight={() =>
          state.setState({ cycleHighlight: !state.cycleHighlight })
        }
        viewMode={state.viewMode}
        pathFinder={state.pathFinder}
        onCancelPathFinder={() => state.setState({ pathFinder: { mode: "idle" } })}
        pathHighlight={pathHighlight}
        minCutHighlight={minCutHighlight}
        issueFocusActive={!!issueFocus}
        onCancelIssueFocus={() => setIssueFocus(null)}
        expandedContainers={issueFocus?.expandedContainers ?? new Set()}
        participantFiles={issueFocus?.participantFiles ?? new Set()}
        selectedEdge={state.selectedEdge}
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
  const fileCount = visibleNodes.filter((n) => n.kind === "file").length;
  const moduleCount = visibleNodes.filter((n) => n.kind !== "file").length;
  const visibleCycles = data.cycles.filter((c) => c.members.every((id) => allScopeIds.has(id)));
  const visibleViolations = data.violations.filter(
    (v) => allScopeIds.has(v.edge.from) && allScopeIds.has(v.edge.to),
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
