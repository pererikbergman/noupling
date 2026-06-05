import type { BreadcrumbSegment } from "../state/explorerState";

export interface BreadcrumbProps {
  segments: BreadcrumbSegment[];
  onSegmentClick: (scope: string) => void;
  onClearScope: () => void;
  codebaseTitle: string;
}

export function Breadcrumb({
  segments,
  onSegmentClick,
  onClearScope,
  codebaseTitle,
}: BreadcrumbProps) {
  if (segments.length === 0) return null;
  return (
    <nav
      aria-label="Drill scope"
      // Full-width inlay row at the top of the canvas. Was floating
      // top-right and overflowing into the spot-filter pills on deep
      // paths (Android repo with ~10 segments).
      className="absolute left-4 right-4 top-3 z-10 flex flex-wrap items-center gap-1 truncate rounded-md border border-border bg-card/95 px-3 py-1.5 text-[12px] backdrop-blur"
    >
      <button
        onClick={onClearScope}
        className="font-medium text-muted hover:text-text"
        title="Clear drill scope"
      >
        {codebaseTitle || "root"}
      </button>
      {segments.map((s, i) => (
        <span key={s.scope} className="flex items-center gap-1">
          <span className="text-muted/60">/</span>
          <button
            onClick={() => onSegmentClick(s.scope)}
            className={
              i === segments.length - 1
                ? "font-semibold text-text"
                : "text-muted hover:text-text"
            }
          >
            {s.label}
          </button>
        </span>
      ))}
    </nav>
  );
}
