import type { DataContract } from "../types";
import { LSM } from "../lsm/LSM";

export function CanvasArea({ data }: { data: DataContract }) {
  return (
    <main id="root-canvas" className="relative overflow-hidden bg-canvas">
      {/* Spot-filter pills overlay on the canvas */}
      <div className="absolute left-4 top-3 z-10 flex flex-wrap gap-1.5">
        <Pill active>All</Pill>
        <Pill>In cycles ({data.summary_counts.cycles})</Pill>
        <Pill>With violations ({data.summary_counts.violations})</Pill>
        <Pill>Clean modules</Pill>
        <Pill>Gravity wells ({data.summary_counts.gravity_wells})</Pill>
      </div>

      <div className="h-full w-full overflow-auto px-4 pb-16 pt-14">
        <LSM data={data} />
      </div>

      {/* Zoom controls (bottom-left) */}
      <div className="absolute bottom-3 left-3 z-10 flex flex-col gap-0.5 rounded-sm border border-border bg-card p-0.5">
        <ZoomBtn title="Zoom in (+)">+</ZoomBtn>
        <ZoomBtn title="Zoom out (−)">−</ZoomBtn>
        <ZoomBtn title="Fit view (1)">⛶</ZoomBtn>
        <ZoomBtn title="Toggle interactivity">⇄</ZoomBtn>
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

function Pill({ active, children }: { active?: boolean; children: React.ReactNode }) {
  return (
    <button
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
}: {
  children: React.ReactNode;
  title: string;
}) {
  return (
    <button
      title={title}
      className="h-6 w-6 rounded-sm text-text hover:bg-pill hover:text-pill-text"
    >
      {children}
    </button>
  );
}
