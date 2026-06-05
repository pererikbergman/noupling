import { useMemo, useState } from "react";
import type { DataContract, NodeEntry } from "../types";
import { LSM } from "../lsm/LSM";
import { Matrix } from "../lsm/Matrix";
import { ForceView } from "../lsm/ForceView";
import { CompositionView } from "../lsm/CompositionView";
import { Breadcrumb } from "./Breadcrumb";
import {
  breadcrumbFor,
  type SpotFilter,
  type ViewMode,
  type PathFinder,
  shouldHighlightViolations,
} from "../state/explorerState";

export interface CanvasAreaProps {
  data: DataContract;
  scope: string;
  codebaseTitle: string;
  onScope: (scope: string) => void;
  onClearScope: () => void;
  onNodeClick?: (id: string) => void;
  spotFilter: SpotFilter;
  onSpotFilter: (f: SpotFilter) => void;
  layerOverlay: boolean;
  onToggleLayerOverlay: () => void;
  cycleHighlight: boolean;
  onToggleCycleHighlight: () => void;
  viewMode: ViewMode;
  pathFinder: PathFinder;
  onCancelPathFinder: () => void;
  pathHighlight: Set<string>;
  minCutHighlight: Set<string>;
  issueFocusActive: boolean;
  onCancelIssueFocus: () => void;
  /** Issue-focus mode: containers to render *expanded* — their
   *  participant files appear inline instead of the container card. */
  expandedContainers: Set<string>;
  /** Issue-focus mode: the actual participant file ids to surface
   *  when a container is expanded (keeps the canvas tight). */
  participantFiles: Set<string>;
  /** Currently selected edge for inspection (LSM clicks). */
  selectedEdge: { from: string; to: string } | null;
  /** Click handler invoked when the user clicks an LSM edge. */
  onEdgeClick: (from: string, to: string) => void;
}

const ZOOM_STEP = 1.2;
const MIN_ZOOM = 0.3;
const MAX_ZOOM = 3;

export function CanvasArea({
  data,
  scope,
  codebaseTitle,
  onScope,
  onClearScope,
  onNodeClick,
  spotFilter,
  onSpotFilter,
  layerOverlay,
  onToggleLayerOverlay,
  cycleHighlight,
  onToggleCycleHighlight,
  viewMode,
  pathFinder,
  onCancelPathFinder,
  pathHighlight,
  minCutHighlight,
  issueFocusActive,
  onCancelIssueFocus,
  expandedContainers,
  participantFiles,
  selectedEdge,
  onEdgeClick,
}: CanvasAreaProps) {
  const segments = breadcrumbFor(scope);
  const [zoom, setZoom] = useState(1);

  // The LSM shows immediate children of the scope so the canvas reads
  // like a composition diagram; the Matrix shows the full scoped set.
  const lsmData = useMemo(() => {
    const childrenByParent = new Map<string | null, NodeEntry[]>();
    for (const n of data.nodes) {
      const k = n.parent ?? null;
      const arr = childrenByParent.get(k);
      if (arr) arr.push(n);
      else childrenByParent.set(k, [n]);
    }
    const immediate = data.nodes.filter((n) => isImmediateChild(n, scope));
    const collapsed = immediate.map((n) =>
      collapseSingletonChain(n, childrenByParent),
    );
    // Issue-focus expansion (#275 follow-up): for containers in the
    // expanded set, replace the single container card with the
    // participant file nodes that live under it. Siblings stay
    // collapsed so the canvas highlights the participants without
    // hairballing on many-file packages.
    const visibleNodes: NodeEntry[] = [];
    for (const n of collapsed) {
      if (expandedContainers.has(n.id) && participantFiles.size > 0) {
        const files = data.nodes.filter(
          (m) =>
            m.kind === "file" &&
            participantFiles.has(m.id) &&
            (m.id === n.id || m.id.startsWith(n.id + "/")),
        );
        if (files.length > 0) {
          visibleNodes.push(...files);
          continue;
        }
      }
      visibleNodes.push(n);
    }
    const visibleIds = new Set(visibleNodes.map((n) => n.id));

    // File-level edges from data.edges connect leaf files, but the
    // visible nodes at this scope are containers/packages. Aggregate
    // every file-level edge up to the visible-ancestor pair so the LSM
    // and Matrix actually show coupling between the cards on screen.
    const ancestorOf = (id: string): string | null => {
      // Self: a visible node is its own representative.
      if (visibleIds.has(id)) return id;
      // Otherwise: find the visible node whose id is a prefix of `id`.
      // visibleNodes is small (capped to immediate children of scope
      // after singleton collapse), so a linear scan is fine here.
      for (const v of visibleNodes) {
        if (id.startsWith(v.id + "/")) return v.id;
      }
      return null;
    };

    const aggregated = new Map<string, { weight: number; violates: string | null }>();
    for (const e of data.edges) {
      const a = ancestorOf(e.from);
      const b = ancestorOf(e.to);
      if (!a || !b || a === b) continue;
      const key = `${a}${b}`;
      const cur = aggregated.get(key);
      if (cur) {
        cur.weight += e.weight;
        if (e.violates_rule && !cur.violates) cur.violates = e.violates_rule;
      } else {
        aggregated.set(key, {
          weight: e.weight,
          violates: e.violates_rule,
        });
      }
    }
    const edges = Array.from(aggregated.entries()).map(([key, v]) => {
      const sep = key.indexOf("");
      return {
        from: key.slice(0, sep),
        to: key.slice(sep + 1),
        weight: v.weight,
        violates_rule: v.violates,
      };
    });

    return {
      ...data,
      nodes: visibleNodes,
      edges,
    };
  }, [data, scope, expandedContainers, participantFiles]);

  function onNodeDoubleClick(id: string) {
    const node = data.nodes.find((n) => n.id === id);
    if (!node) return;
    if (node.kind === "file") return;
    onScope(node.id);
  }

  const cyclesByNode = new Map<string, number>();
  for (const c of data.cycles) {
    for (const id of c.members) {
      cyclesByNode.set(id, (cyclesByNode.get(id) ?? 0) + 1);
    }
  }

  return (
    <main id="root-canvas" className="relative overflow-hidden bg-canvas">
      {/* Spot-filter pills overlay on the canvas (PRD F5.5). Sits
          below the breadcrumb row; top offset accounts for it. */}
      <div className="absolute left-4 top-14 z-10 flex flex-wrap gap-1.5">
        <FilterPill active={spotFilter === "all"} onClick={() => onSpotFilter("all")}>
          All
        </FilterPill>
        <FilterPill
          active={spotFilter === "in-cycles"}
          onClick={() => onSpotFilter("in-cycles")}
        >
          In cycles ({data.summary_counts.cycles})
        </FilterPill>
        <FilterPill
          active={spotFilter === "with-violations"}
          onClick={() => onSpotFilter("with-violations")}
        >
          With violations ({data.summary_counts.violations})
        </FilterPill>
        <FilterPill active={spotFilter === "clean"} onClick={() => onSpotFilter("clean")}>
          Clean modules
        </FilterPill>
        <FilterPill
          active={spotFilter === "hide-violations"}
          onClick={() => onSpotFilter("hide-violations")}
        >
          Hide violations
        </FilterPill>
        <FilterPill
          active={spotFilter === "gravity-wells"}
          onClick={() => onSpotFilter("gravity-wells")}
        >
          Gravity wells ({data.summary_counts.gravity_wells})
        </FilterPill>
      </div>

      <Breadcrumb
        segments={segments}
        onSegmentClick={onScope}
        onClearScope={onClearScope}
        codebaseTitle={codebaseTitle}
      />

      {pathFinder.mode !== "idle" && (
        <PathFinderBanner pathFinder={pathFinder} onCancel={onCancelPathFinder} />
      )}

      {issueFocusActive && (
        <div
          role="status"
          className="absolute left-4 right-4 top-24 z-20 flex items-center justify-between rounded-md border border-edge-violation/40 bg-edge-violation/10 px-3 py-2 text-[12px]"
        >
          <span>
            <strong className="text-edge-violation">Issue focused.</strong>{" "}
            Canvas scoped to the participants; offending edges highlighted.
          </span>
          <button
            onClick={onCancelIssueFocus}
            aria-label="Exit issue focus mode"
            className="rounded-sm px-2 py-0.5 text-[11px] text-muted transition-colors hover:bg-canvas/60 hover:text-text"
          >
            Exit (Esc)
          </button>
        </div>
      )}

      {viewMode === "lsm" && (
        <div className="h-full w-full overflow-auto px-4 pb-16 pt-24">
          <div
            style={{
              transform: `scale(${zoom})`,
              transformOrigin: "top left",
              display: "inline-block",
              minWidth: "100%",
            }}
          >
            <LSM
              data={lsmData}
              onNodeClick={onNodeClick}
              onNodeDoubleClick={onNodeDoubleClick}
              highlightViolations={shouldHighlightViolations(spotFilter)}
              highlightCycles={cycleHighlight}
              layerOverlay={layerOverlay}
              cyclesByNode={cyclesByNode}
              pathHighlight={pathHighlight}
              minCutHighlight={minCutHighlight}
              selectedEdgeKey={
                selectedEdge
                  ? `${selectedEdge.from}→${selectedEdge.to}`
                  : null
              }
              onEdgeClick={onEdgeClick}
            />
          </div>
        </div>
      )}
      {viewMode === "matrix" && (
        <div className="h-full w-full overflow-auto px-4 pb-16 pt-24">
          <Matrix data={lsmData} onNodeClick={onNodeClick} />
        </div>
      )}
      {viewMode === "force" && (
        <div className="h-full w-full px-4 pb-16 pt-24">
          <ForceView
            nodes={lsmData.nodes}
            edges={lsmData.edges}
            clusters={data.clusters}
            onNodeClick={onNodeClick}
          />
        </div>
      )}
      {viewMode === "composition" && (
        <div className="h-full w-full pt-24">
          <CompositionView
            nodes={lsmData.nodes}
            edges={lsmData.edges}
            onNodeClick={onNodeClick}
          />
        </div>
      )}

      {/* Toolbar (bottom-left) */}
      <div className="absolute bottom-3 left-3 z-10 flex flex-col gap-0.5 rounded-sm border border-border bg-card p-0.5">
        <ZoomBtn
          title={`Zoom in (${Math.round(zoom * 100)}%)`}
          ariaLabel="Zoom in"
          onClick={() => setZoom((z) => Math.min(MAX_ZOOM, z * ZOOM_STEP))}
        >
          +
        </ZoomBtn>
        <ZoomBtn
          title={`Zoom out (${Math.round(zoom * 100)}%)`}
          ariaLabel="Zoom out"
          onClick={() => setZoom((z) => Math.max(MIN_ZOOM, z / ZOOM_STEP))}
        >
          −
        </ZoomBtn>
        <ZoomBtn
          title="Reset zoom to 100%"
          ariaLabel="Reset zoom"
          onClick={() => setZoom(1)}
        >
          ⛶
        </ZoomBtn>
        <ZoomBtn
          title={layerOverlay ? "Hide layer overlay" : "Show layer overlay"}
          ariaLabel={layerOverlay ? "Hide layer-identity overlay" : "Show layer-identity overlay"}
          active={layerOverlay}
          onClick={onToggleLayerOverlay}
        >
          L
        </ZoomBtn>
        <ZoomBtn
          title={cycleHighlight ? "Hide cycle highlights" : "Show cycle highlights"}
          ariaLabel={cycleHighlight ? "Hide cycle highlights" : "Show cycle highlights"}
          active={cycleHighlight}
          onClick={onToggleCycleHighlight}
        >
          ⊚
        </ZoomBtn>
      </div>
    </main>
  );
}

function isImmediateChild(node: NodeEntry, scope: string): boolean {
  if (scope === "") return node.parent === null;
  return node.parent === scope;
}

function collapseSingletonChain(
  start: NodeEntry,
  childrenByParent: Map<string | null, NodeEntry[]>,
): NodeEntry {
  if (start.kind === "file") return start;
  let current = start;
  while (true) {
    const children = childrenByParent.get(current.id) ?? [];
    if (children.length !== 1) return current;
    const only = children[0];
    if (only.kind === "file") return current;
    current = only;
  }
}

function PathFinderBanner({
  pathFinder,
  onCancel,
}: {
  pathFinder: PathFinder;
  onCancel: () => void;
}) {
  let msg: string;
  switch (pathFinder.mode) {
    case "pick-from":
      msg = "Path finder — click the start node";
      break;
    case "pick-to":
      msg = `Path finder — click the destination (start: ${basename(pathFinder.from)})`;
      break;
    case "done":
      msg = `Path: ${basename(pathFinder.from)} → ${basename(pathFinder.to)}`;
      break;
    default:
      return null;
  }
  return (
    <div className="absolute left-1/2 top-24 z-20 flex -translate-x-1/2 items-center gap-3 rounded-full border border-accent-ui bg-card px-3 py-1.5 text-[12px] text-text shadow-md">
      <span className="font-mono text-accent-ui">↣</span>
      <span>{msg}</span>
      <button
        onClick={onCancel}
        className="rounded-sm px-2 py-0.5 text-[11px] text-muted hover:bg-canvas hover:text-text"
      >
        Cancel · esc
      </button>
    </div>
  );
}

function basename(p: string): string {
  return p.split("/").filter(Boolean).pop() ?? p;
}

function FilterPill({
  active,
  children,
  onClick,
}: {
  active?: boolean;
  children: React.ReactNode;
  onClick?: () => void;
}) {
  return (
    <button
      onClick={onClick}
      className={
        "rounded-full border px-3 py-1 text-[11px] " +
        (active
          ? "border-pill bg-pill text-pill-text"
          : "border-border text-muted hover:text-text")
      }
    >
      {children}
    </button>
  );
}

function ZoomBtn({
  children,
  title,
  ariaLabel,
  active,
  onClick,
}: {
  children: React.ReactNode;
  title: string;
  ariaLabel?: string;
  active?: boolean;
  onClick?: () => void;
}) {
  return (
    <button
      title={title}
      aria-label={ariaLabel ?? title}
      aria-pressed={active}
      onClick={onClick}
      className={
        "h-6 w-6 rounded-sm " +
        (active ? "bg-pill text-pill-text" : "text-text hover:bg-pill hover:text-pill-text")
      }
    >
      {children}
    </button>
  );
}
