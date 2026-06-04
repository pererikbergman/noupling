import { useMemo, useState } from "react";
import type { DataContract, NodeEntry } from "../types";
import { LSM } from "../lsm/LSM";
import { Breadcrumb } from "./Breadcrumb";
import {
  breadcrumbFor,
  type SpotFilter,
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
}: CanvasAreaProps) {
  const segments = breadcrumbFor(scope);
  const [zoom, setZoom] = useState(1);

  // Filter the data passed to the LSM down to *immediate children* of the
  // current scope so the canvas reads as a Structure101-style composition
  // diagram: at root you see top-level packages, drill in to see their
  // children, drill again to land on files. The side panel + search row
  // still show counts over the full sub-tree (PRD F3.3).
  const lsmData = useMemo(() => {
    const childrenByParent = new Map<string | null, NodeEntry[]>();
    for (const n of data.nodes) {
      const k = n.parent ?? null;
      const arr = childrenByParent.get(k);
      if (arr) arr.push(n);
      else childrenByParent.set(k, [n]);
    }
    const immediate = data.nodes.filter((n) => isImmediateChild(n, scope));
    // Collapse singleton chains: when an immediate child is a directory
    // with exactly one directory child (and no files of its own), jump to
    // that child. Repeat until the displayed node is a fork (>1 child),
    // a directory that contains a file directly, or a leaf file. The
    // breadcrumb is still the canonical navigation surface for any
    // intermediate level you want to jump back to.
    const visibleNodes = immediate.map((n) => collapseSingletonChain(n, childrenByParent));
    const visibleIds = new Set(visibleNodes.map((n) => n.id));
    return {
      ...data,
      nodes: visibleNodes,
      edges: data.edges.filter(
        (e) => visibleIds.has(e.from) && visibleIds.has(e.to),
      ),
    };
  }, [data, scope]);

  function onNodeDoubleClick(id: string) {
    const node = data.nodes.find((n) => n.id === id);
    if (!node) return;
    if (node.kind === "file") return; // leaves don't drill
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
      {/* Spot-filter pills overlay on the canvas (PRD F5.5) */}
      <div className="absolute left-4 top-3 z-10 flex flex-wrap gap-1.5">
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

      <div className="h-full w-full overflow-auto px-4 pb-16 pt-14">
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
          />
        </div>
      </div>

      {/* Zoom + view-mode controls (bottom-left) */}
      <div className="absolute bottom-3 left-3 z-10 flex flex-col gap-0.5 rounded-sm border border-border bg-card p-0.5">
        <ZoomBtn
          title={`Zoom in (currently ${Math.round(zoom * 100)}%)`}
          ariaLabel="Zoom in"
          onClick={() => setZoom((z) => Math.min(MAX_ZOOM, z * ZOOM_STEP))}
        >
          +
        </ZoomBtn>
        <ZoomBtn
          title={`Zoom out (currently ${Math.round(zoom * 100)}%)`}
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

      {/* PLAN strip — v2 placeholder, visibly disabled so it can't be
          mistaken for a wired affordance (#254). */}
      <div
        className="absolute bottom-3 right-4 z-10 flex cursor-not-allowed items-center gap-3 rounded-md border border-dashed border-border bg-card/60 px-3.5 py-2.5 text-[12px] text-muted opacity-60"
        title="Action plan ships in v2 (#228 §9)"
        aria-disabled="true"
      >
        <span className="rounded-full bg-text/40 px-2 py-0.5 text-[10px] font-bold text-canvas">
          PLAN
        </span>
        <span>
          Refactor sandbox · <strong className="text-muted">v2</strong>
        </span>
      </div>
    </main>
  );
}

function isImmediateChild(node: NodeEntry, scope: string): boolean {
  if (scope === "") return node.parent === null;
  return node.parent === scope;
}

/**
 * Walk down a singleton chain — keeps descending as long as the current
 * directory has exactly one *directory* child and no files of its own.
 * Stops at the first fork (≥2 children) or the first dir that holds a
 * file directly, or a leaf file.
 *
 * For an Android `app/src/main/java/com/<org>/<app>/cart/...` codebase
 * the root view collapses `app` straight to the first package with
 * multiple sub-packages, skipping the seven dead-end intermediate
 * directories.
 */
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
