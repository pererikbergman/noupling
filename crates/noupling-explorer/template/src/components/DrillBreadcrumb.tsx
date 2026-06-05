import { breadcrumbFor, parentDir } from "../state/explorerState";

export interface DrillBreadcrumbProps {
  scope: string;
  onScope: (scope: string) => void;
  rootLabel?: string;
}

/**
 * Shared up/breadcrumb header for side-panel tabs that mutate the
 * global drill scope (#273 Files, #274 Levels). Renders nothing at
 * root scope so it doesn't take up space when there's nowhere to go.
 */
// Maximum segments to render after the root before we collapse the
// middle to `…`. Tuned so the breadcrumb always fits inside the
// 380-px side panel without horizontal scroll, even at the deep
// Android-style scope `app/src/main/java/com/hlpth/.../data`.
const VISIBLE_TAIL = 2;

export function DrillBreadcrumb({
  scope,
  onScope,
  rootLabel = "root",
}: DrillBreadcrumbProps) {
  if (scope === "") return null;
  const crumbs = breadcrumbFor(scope);
  const up = parentDir(scope);

  // When the path is deep, keep the root anchor + last few segments
  // visible and collapse the middle to an ellipsis. The ellipsis
  // itself is clickable so the user can jump to the first hidden
  // segment if they want to inspect the middle.
  const showEllipsis = crumbs.length > VISIBLE_TAIL + 1;
  const hiddenCount = showEllipsis ? crumbs.length - VISIBLE_TAIL : 0;
  const visibleCrumbs = showEllipsis ? crumbs.slice(-VISIBLE_TAIL) : crumbs;
  const firstHidden = showEllipsis ? crumbs[hiddenCount - 1] : null;

  return (
    <div className="mb-2 flex min-w-0 items-center gap-1 rounded-sm border border-border bg-canvas px-2 py-1.5 text-[11px]">
      <button
        onClick={() => onScope(up)}
        aria-label={up === "" ? `Up to ${rootLabel}` : `Up to ${up}`}
        className="shrink-0 rounded-sm px-1 text-muted hover:bg-pill hover:text-pill-text"
        title="Go up one level"
      >
        ↑
      </button>
      <button
        onClick={() => onScope("")}
        className="shrink-0 font-mono text-muted hover:text-text"
        title={`Back to ${rootLabel}`}
      >
        {rootLabel}
      </button>
      {showEllipsis && firstHidden && (
        <span className="flex shrink-0 items-center gap-1">
          <span className="text-muted/50">/</span>
          <button
            onClick={() => onScope(firstHidden.scope)}
            className="font-mono text-muted hover:text-text"
            title={`Jump to ${firstHidden.scope} (${hiddenCount} hidden segment${
              hiddenCount === 1 ? "" : "s"
            })`}
          >
            …
          </button>
        </span>
      )}
      {visibleCrumbs.map((c, i) => (
        <span key={c.scope} className="flex min-w-0 items-center gap-1">
          <span className="shrink-0 text-muted/50">/</span>
          <button
            onClick={() => onScope(c.scope)}
            disabled={i === visibleCrumbs.length - 1}
            className={
              "truncate font-mono " +
              (i === visibleCrumbs.length - 1
                ? "text-text"
                : "text-muted hover:text-text")
            }
          >
            {c.label}
          </button>
        </span>
      ))}
    </div>
  );
}
