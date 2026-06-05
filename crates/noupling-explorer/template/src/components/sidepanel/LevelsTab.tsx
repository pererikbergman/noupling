import { useMemo } from "react";
import type { DataContract, NodeEntry } from "../../types";
import { DrillBreadcrumb } from "../DrillBreadcrumb";
import { basename, layerAccent } from "./shared";

/**
 * #274 — Finder-style one-level-at-a-time drill-down. Shows the
 * immediate container children of the current shared scope, with a
 * trimmed badge row per node (file count + active violation count).
 * Single click selects (opens DetailsPanel); double-click drills the
 * shared scope. Up/breadcrumb shares <DrillBreadcrumb> with Files.
 */
export function LevelsTab({
  data,
  scope,
  onScope,
  onSelect,
}: {
  data: DataContract;
  scope: string;
  onScope?: (scope: string) => void;
  onSelect?: (id: string) => void;
}) {
  const childrenByParent = useMemo(() => {
    const m = new Map<string | null, NodeEntry[]>();
    for (const n of data.nodes) {
      const key = n.parent;
      const arr = m.get(key);
      if (arr) arr.push(n);
      else m.set(key, [n]);
    }
    for (const arr of m.values()) arr.sort((a, b) => a.id.localeCompare(b.id));
    return m;
  }, [data.nodes]);

  // Per-container violation count = number of violations whose
  // offender file sits inside the container. Cheap to compute lazily.
  const violationsByContainer = useMemo(() => {
    const counts = new Map<string, number>();
    for (const v of data.violations) {
      // Walk up the parent chain from the offender's file and credit
      // every ancestor container. Stops when no parent found.
      let cur: string | null = v.edge.from;
      while (cur) {
        counts.set(cur, (counts.get(cur) ?? 0) + 1);
        const node = data.nodes.find((n) => n.id === cur);
        cur = node?.parent ?? null;
      }
    }
    return counts;
  }, [data.violations, data.nodes]);

  const rootKey = scope === "" ? null : scope;
  const children = (childrenByParent.get(rootKey) ?? []).filter(
    (n) => n.kind !== "file",
  );

  return (
    <div>
      <DrillBreadcrumb scope={scope} onScope={(s) => onScope?.(s)} />
      <p className="m-0 mb-2 text-[11px] text-muted">
        One level at a time. Click a row to select; double-click to drill.
      </p>
      {children.length === 0 ? (
        <p className="m-0 text-[12px] text-muted">
          No nested containers at this level.
        </p>
      ) : (
        <ul className="m-0 flex list-none flex-col gap-1 p-0">
          {children.map((n) => {
            const fc =
              typeof n.metrics.file_count === "number"
                ? n.metrics.file_count
                : null;
            const violCount = violationsByContainer.get(n.id) ?? 0;
            return (
              <li key={n.id}>
                <button
                  onClick={() => onSelect?.(n.id)}
                  onDoubleClick={() => onScope?.(n.id)}
                  className="flex w-full items-center justify-between rounded-sm border border-border bg-canvas px-2 py-1.5 text-left text-[12px] transition-colors hover:bg-canvas/60 hover:border-text/30"
                  title={`${n.id} — double-click to drill`}
                >
                  <span className="flex min-w-0 items-center gap-2">
                    <span
                      className={
                        "inline-block h-3 w-0.5 rounded-sm align-middle " +
                        layerAccent(n.layer)
                      }
                    />
                    <span className="truncate font-mono text-text">
                      {basename(n.id)}
                    </span>
                  </span>
                  <span className="ml-2 flex shrink-0 items-center gap-2 text-[10px]">
                    {fc !== null && (
                      <span className="text-muted">{fc}f</span>
                    )}
                    {violCount > 0 && (
                      <span className="rounded-full bg-edge-violation/20 px-1.5 py-0.5 font-bold text-edge-violation">
                        {violCount}
                      </span>
                    )}
                  </span>
                </button>
              </li>
            );
          })}
        </ul>
      )}
    </div>
  );
}
