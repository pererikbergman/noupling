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
export function DrillBreadcrumb({
  scope,
  onScope,
  rootLabel = "root",
}: DrillBreadcrumbProps) {
  if (scope === "") return null;
  const crumbs = breadcrumbFor(scope);
  const up = parentDir(scope);

  return (
    <div className="mb-2 flex items-center gap-1 rounded-sm border border-border bg-canvas px-2 py-1.5 text-[11px]">
      <button
        onClick={() => onScope(up)}
        aria-label={up === "" ? `Up to ${rootLabel}` : `Up to ${up}`}
        className="rounded-sm px-1 text-muted hover:bg-pill hover:text-pill-text"
        title="Go up one level"
      >
        ↑
      </button>
      <button
        onClick={() => onScope("")}
        className="font-mono text-muted hover:text-text"
        title={`Back to ${rootLabel}`}
      >
        {rootLabel}
      </button>
      {crumbs.map((c, i) => (
        <span key={c.scope} className="flex items-center gap-1">
          <span className="text-muted/50">/</span>
          <button
            onClick={() => onScope(c.scope)}
            disabled={i === crumbs.length - 1}
            className={
              "font-mono " +
              (i === crumbs.length - 1
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
