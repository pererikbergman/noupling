import { useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import type { DataContract, NodeEntry } from "../types";
import { LSM } from "../lsm/LSM";
import { computeLSMLayout } from "../lsm/layout";
import { Matrix } from "../lsm/Matrix";
import { CompositionView } from "../lsm/CompositionView";
import { Breadcrumb } from "./Breadcrumb";
import {
  breadcrumbFor,
  type SpotFilter,
  type ViewMode,
  type PathFinder,
} from "../state/explorerState";
import { nodeById } from "../state/queries";
import { collapsedLabel } from "../state/labels";
import type { HighlightPolicy } from "../state/highlightPolicy";

export interface CanvasAreaProps {
  data: DataContract;
  scope: string;
  codebaseTitle: string;
  onScope: (scope: string) => void;
  onClearScope: () => void;
  onNodeClick?: (id: string) => void;
  spotFilter: SpotFilter;
  onSpotFilter: (f: SpotFilter) => void;
  onToggleLayerOverlay: () => void;
  onToggleCycleHighlight: () => void;
  viewMode: ViewMode;
  pathFinder: PathFinder;
  onCancelPathFinder: () => void;
  onCancelIssueFocus: () => void;
  /** Resolved canvas highlight rules built once in App.tsx. */
  highlight: HighlightPolicy;
  /** Click handler invoked when the user clicks an LSM edge. */
  onEdgeClick: (from: string, to: string) => void;
}

const ZOOM_STEP = 1.2;
const MIN_ZOOM = 0.3;
const MAX_ZOOM = 3;
/** Fit never enlarges a small graph past this — one node must not fill the screen (#397, #399). */
const MAX_FIT_ZOOM = 1.15;
/** Left gutter so the zoom stack never covers the first column or a band label (#399). */
const CANVAS_GUTTER_CLASS = "pl-14";
/** Matrix cells scale with the canvas between these bounds (#399). */
const MATRIX_CELL_MIN = 14;
const MATRIX_CELL_MAX = 36;
const MATRIX_LABEL_COLUMN = 220;
// Stable empty Set so issue-focus-off renders don't churn child memos
// keyed on identity.
const EMPTY_STRING_SET: Set<string> = new Set();

export function CanvasArea({
  data,
  scope,
  codebaseTitle,
  onScope,
  onClearScope,
  onNodeClick,
  spotFilter,
  onSpotFilter,
  onToggleLayerOverlay,
  onToggleCycleHighlight,
  viewMode,
  pathFinder,
  onCancelPathFinder,
  onCancelIssueFocus,
  highlight,
  onEdgeClick,
}: CanvasAreaProps) {
  const segments = breadcrumbFor(scope);
  const [zoom, setZoom] = useState(1);
  const canvasRef = useRef<HTMLElement>(null);
  const canvasSize = useElementSize(canvasRef);
  const expandedContainers =
    highlight.issueFocus?.expandedContainers ?? EMPTY_STRING_SET;
  const participantFiles =
    highlight.issueFocus?.participantFiles ?? EMPTY_STRING_SET;
  const issueFocusActive = highlight.issueFocus !== null;
  // The issue-focus banner is absolutely positioned at top-24, exactly where
  // the first tier label renders. Push the canvas down while it's shown so
  // the banner has its own row instead of overprinting that label.
  const canvasTop = issueFocusActive ? "pt-36" : "pt-24";

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
      collapsedLabel(n, collapseSingletonChain(n, childrenByParent)),
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

  // Fit the LSM to the canvas whenever what it shows changes (scope,
  // view, focus, filters). Manual zooming still works afterwards; the
  // fit button re-applies this (#399).
  const lsmExtent = useMemo(() => {
    const l = computeLSMLayout(lsmData);
    return { width: l.width, height: l.height };
  }, [lsmData]);
  const fitZoom = useMemo(() => {
    if (canvasSize.width === 0 || canvasSize.height === 0) return 1;
    const topOffset = issueFocusActive ? 144 : 96; // pt-36 / pt-24
    const availW = canvasSize.width - 56 - 32; // gutter + right padding
    const availH = canvasSize.height - topOffset - 64; // bottom padding
    const z = Math.min(availW / lsmExtent.width, availH / lsmExtent.height);
    return Math.max(MIN_ZOOM, Math.min(MAX_FIT_ZOOM, z));
  }, [canvasSize, lsmExtent, issueFocusActive]);
  const sizeReady = canvasSize.width > 0;
  useEffect(() => {
    if (sizeReady) setZoom(fitZoom);
    // Re-fit on content change and the first measured size — not on
    // every resize, so a manual zoom survives opening the details panel.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [scope, viewMode, lsmData, sizeReady]);

  const matrixCell = useMemo(() => {
    const n = Math.max(1, lsmData.nodes.length);
    const avail = canvasSize.width - MATRIX_LABEL_COLUMN - 56 - 48;
    return Math.max(MATRIX_CELL_MIN, Math.min(MATRIX_CELL_MAX, Math.floor(avail / n)));
  }, [canvasSize.width, lsmData.nodes.length]);

  function onNodeDoubleClick(id: string) {
    const node = nodeById(data, id);
    if (!node) return;
    if (node.kind === "file") return;
    onScope(node.id);
  }

  return (
    <main id="root-canvas" ref={canvasRef} className="relative overflow-hidden bg-canvas">
      {/* Spot-filter pills overlay on the canvas (PRD F5.5). Sits
          below the breadcrumb row; top offset accounts for it. */}
      <div className="absolute left-4 top-14 z-10 flex flex-wrap gap-1.5">
        <FilterPill
          active={spotFilter === "all"}
          onClick={() => onSpotFilter("all")}
          title="Show every node in scope. Default."
        >
          All
        </FilterPill>
        <FilterPill
          active={spotFilter === "in-cycles"}
          onClick={() => onSpotFilter("in-cycles")}
          title="Show only nodes that participate in at least one cycle. Use this to see how cycles span the architecture."
        >
          In cycles ({data.summary_counts.cycles})
        </FilterPill>
        <FilterPill
          active={spotFilter === "with-violations"}
          onClick={() => onSpotFilter("with-violations")}
          title="Show only nodes touched by at least one rule violation."
        >
          With violations ({data.summary_counts.violations})
        </FilterPill>
        <FilterPill
          active={spotFilter === "clean"}
          onClick={() => onSpotFilter("clean")}
          title="Show only nodes with zero violations and no cycle membership — the healthy parts of the codebase."
        >
          Clean modules
        </FilterPill>
        <FilterPill
          active={spotFilter === "hide-violations"}
          onClick={() => onSpotFilter("hide-violations")}
          title="Keep all nodes but mute the red violation highlights, so the structural shape reads cleanly."
        >
          Hide violations
        </FilterPill>
        <FilterPill
          active={spotFilter === "gravity-wells"}
          onClick={() => onSpotFilter("gravity-wells")}
          title="Show only modules flagged as gravity wells — disproportionately high aggregate RRI. Concentration risk for refactors."
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
        <div className={`h-full w-full overflow-auto pr-4 pb-16 ${CANVAS_GUTTER_CLASS} ${canvasTop}`}>
          {/* The outer box takes the scaled size so the scroll extent matches
              what is drawn; the inner box carries the transform. */}
          <div
            style={{
              width: lsmExtent.width * zoom,
              height: lsmExtent.height * zoom,
            }}
          >
            <div
              style={{
                transform: `scale(${zoom})`,
                transformOrigin: "top left",
                width: lsmExtent.width,
                height: lsmExtent.height,
              }}
            >
            <LSM
              data={lsmData}
              onNodeClick={onNodeClick}
              onNodeDoubleClick={onNodeDoubleClick}
              highlight={highlight}
              onEdgeClick={onEdgeClick}
            />
            </div>
          </div>
        </div>
      )}
      {viewMode === "matrix" && (
        <div className={`h-full w-full overflow-auto pr-4 pb-16 ${CANVAS_GUTTER_CLASS} ${canvasTop}`}>
          <Matrix
            data={lsmData}
            cellSize={matrixCell}
            onNodeClick={onNodeClick}
            onNodeDoubleClick={onNodeDoubleClick}
            highlight={highlight}
            onEdgeClick={onEdgeClick}
          />
        </div>
      )}
      {viewMode === "composition" && (
        <div className={`h-full w-full ${CANVAS_GUTTER_CLASS} ${canvasTop}`}>
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
          title={`Fit to view (${Math.round(fitZoom * 100)}%)`}
          ariaLabel="Fit to view"
          onClick={() => setZoom(fitZoom)}
        >
          ⛶
        </ZoomBtn>
        <ZoomBtn
          title={highlight.layerOverlay ? "Hide layer overlay" : "Show layer overlay"}
          ariaLabel={highlight.layerOverlay ? "Hide layer-identity overlay" : "Show layer-identity overlay"}
          active={highlight.layerOverlay}
          onClick={onToggleLayerOverlay}
        >
          L
        </ZoomBtn>
        <ZoomBtn
          title={highlight.highlightCycles ? "Hide cycle highlights" : "Show cycle highlights"}
          ariaLabel={highlight.highlightCycles ? "Hide cycle highlights" : "Show cycle highlights"}
          active={highlight.highlightCycles}
          onClick={onToggleCycleHighlight}
        >
          ⊚
        </ZoomBtn>
      </div>
    </main>
  );
}

/** Live size of an element, via ResizeObserver; `{0,0}` until measured. */
function useElementSize(ref: React.RefObject<HTMLElement | null>) {
  const [size, setSize] = useState({ width: 0, height: 0 });
  useLayoutEffect(() => {
    const el = ref.current;
    if (!el) return;
    const update = () =>
      setSize({ width: el.clientWidth, height: el.clientHeight });
    update();
    if (typeof ResizeObserver === "undefined") return;
    const ro = new ResizeObserver(update);
    ro.observe(el);
    return () => ro.disconnect();
  }, [ref]);
  return size;
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
  title,
}: {
  active?: boolean;
  children: React.ReactNode;
  onClick?: () => void;
  title?: string;
}) {
  return (
    <button
      onClick={onClick}
      title={title}
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
