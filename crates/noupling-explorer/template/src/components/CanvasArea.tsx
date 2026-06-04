import type { DataContract } from "../types";
import { LSM } from "../lsm/LSM";
import { Breadcrumb } from "./Breadcrumb";
import {
  breadcrumbFor,
  parentDir,
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

  function onNodeDoubleClick(id: string) {
    const next = parentDir(id);
    if (next !== "" && next !== scope) {
      onScope(next);
    }
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
        <FilterPill
          active={spotFilter === "clean"}
          onClick={() => onSpotFilter("clean")}
        >
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
        <LSM
          data={data}
          onNodeClick={onNodeClick}
          onNodeDoubleClick={onNodeDoubleClick}
          highlightViolations={shouldHighlightViolations(spotFilter)}
          highlightCycles={cycleHighlight}
          layerOverlay={layerOverlay}
          cyclesByNode={cyclesByNode}
        />
      </div>

      {/* Zoom + view-mode controls (bottom-left) */}
      <div className="absolute bottom-3 left-3 z-10 flex flex-col gap-0.5 rounded-sm border border-border bg-card p-0.5">
        <ZoomBtn title="Zoom in (+)" ariaLabel="Zoom in">
          +
        </ZoomBtn>
        <ZoomBtn title="Zoom out (−)" ariaLabel="Zoom out">
          −
        </ZoomBtn>
        <ZoomBtn title="Fit view (1)" ariaLabel="Fit view">
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

      {/* Action-plan strip (bottom-right) */}
      <div className="absolute bottom-3 right-4 z-10 flex items-center gap-3 rounded-md border border-border bg-card px-3.5 py-2.5 text-[12px] text-muted">
        <span className="rounded-full bg-text/85 px-2 py-0.5 text-[10px] font-bold text-canvas">
          PLAN
        </span>
        <span>
          <strong className="text-text">0</strong> queued
        </span>
        <button className="rounded-sm border border-border px-2.5 py-1 text-[11px] hover:text-text">
          Open
        </button>
      </div>
    </main>
  );
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
