import { useEffect, useMemo } from "react";
import type { DataContract, EdgeEntry } from "../types";
import type { EdgeSelection } from "../state/explorerState";

export interface EdgeDetailsPanelProps {
  data: DataContract;
  /** Edge selected on the canvas, or null. The panel renders into the
   *  App's right grid column whenever this is non-null. */
  selectedEdge: EdgeSelection | null;
  onClose: () => void;
  onSelectNode: (id: string) => void;
  onScope: (scope: string) => void;
}

/**
 * Inlay column shown in place of (or next to) the node DetailsPanel
 * when the user clicks an edge on the LSM. Surfaces the from/to
 * endpoints with click-to-select, the import weight, any rule
 * violation, cycle membership, and — when the selected edge is an
 * aggregated container-to-container edge — the underlying file-level
 * imports that contributed to it.
 */
export function EdgeDetailsPanel({
  data,
  selectedEdge,
  onClose,
  onSelectNode,
  onScope,
}: EdgeDetailsPanelProps) {
  useEffect(() => {
    if (!selectedEdge) return;
    function onKey(e: KeyboardEvent) {
      if (e.key === "Escape") onClose();
    }
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [selectedEdge, onClose]);

  // Look the edge up in the visible (aggregated) edge set first, then
  // collect the file-level contributors from the full Data Contract.
  const aggregate = useMemo(() => {
    if (!selectedEdge) return null;
    return (
      data.edges.find(
        (e) => e.from === selectedEdge.from && e.to === selectedEdge.to,
      ) ?? null
    );
  }, [data.edges, selectedEdge]);

  const fileContributors = useMemo<EdgeEntry[]>(() => {
    if (!selectedEdge) return [];
    // For container-to-container edges, the file-level breakdown is
    // every edge whose from sits inside `selectedEdge.from` AND to
    // sits inside `selectedEdge.to`. For file-to-file edges this
    // collapses to just the edge itself.
    const fromPrefix = selectedEdge.from + "/";
    const toPrefix = selectedEdge.to + "/";
    return data.edges.filter(
      (e) =>
        (e.from === selectedEdge.from || e.from.startsWith(fromPrefix)) &&
        (e.to === selectedEdge.to || e.to.startsWith(toPrefix)),
    );
  }, [data.edges, selectedEdge]);

  const cycle = useMemo(() => {
    if (!selectedEdge) return null;
    return (
      data.cycles.find((c) => {
        for (let i = 0; i < c.members.length; i++) {
          const a = c.members[i];
          const b = c.members[(i + 1) % c.members.length];
          if (a === selectedEdge.from && b === selectedEdge.to) return true;
        }
        return false;
      }) ?? null
    );
  }, [data.cycles, selectedEdge]);

  const violation = useMemo(() => {
    if (!selectedEdge) return null;
    return (
      data.violations.find(
        (v) =>
          v.edge.from === selectedEdge.from && v.edge.to === selectedEdge.to,
      ) ?? null
    );
  }, [data.violations, selectedEdge]);

  if (!selectedEdge) return null;

  return (
    <aside
      role="complementary"
      aria-label={`Details for edge ${selectedEdge.from} → ${selectedEdge.to}`}
      className="flex min-h-0 flex-col border-l border-border bg-card"
    >
      <header className="flex items-center justify-between border-b border-border px-4 py-3">
        <span className="rounded-full bg-accent-domain/15 px-2 py-0.5 text-[10px] font-semibold uppercase tracking-wider text-accent-domain">
          edge
        </span>
        <button
          onClick={onClose}
          aria-label="Close edge details"
          className="rounded-sm px-2 py-1 text-[13px] text-muted hover:bg-canvas hover:text-text"
        >
          ✕ <span className="text-[10px] text-muted/60">esc</span>
        </button>
      </header>

      <div className="flex-1 space-y-4 overflow-y-auto px-4 py-3">
        <section>
          <h2 className="m-0 text-[14px] font-semibold leading-snug">
            {basename(selectedEdge.from)} → {basename(selectedEdge.to)}
          </h2>
          <p className="m-0 mt-1 font-mono text-[10px] text-muted">
            {selectedEdge.from} → {selectedEdge.to}
          </p>
          {aggregate && (
            <p className="m-0 mt-2 text-[12px] text-muted">
              <strong className="text-text">{aggregate.weight}</strong>{" "}
              import{aggregate.weight === 1 ? "" : "s"} across this edge.
            </p>
          )}
        </section>

        {violation && (
          <section className="rounded-sm border border-edge-violation/40 bg-edge-violation/10 p-2.5 text-[12px]">
            <p className="m-0 mb-1 text-[10px] font-bold uppercase tracking-wider text-edge-violation">
              Violates rule
            </p>
            <p className="m-0 font-mono text-[11px] text-text">
              {violation.rule.from} → {violation.rule.to}
            </p>
            <p className="m-0 mt-0.5 text-[11px] text-muted">
              severity: {violation.severity}
            </p>
          </section>
        )}

        {cycle && (
          <section className="rounded-sm border border-edge-cycle/40 bg-edge-cycle/10 p-2.5 text-[12px]">
            <p className="m-0 mb-1 text-[10px] font-bold uppercase tracking-wider text-edge-cycle">
              Part of a cycle
            </p>
            <p className="m-0 font-mono text-[11px] text-text">
              {cycle.id} · {cycle.size} members
            </p>
            <p className="m-0 mt-0.5 truncate font-mono text-[10px] text-muted">
              {cycle.members.join(" → ")}
            </p>
          </section>
        )}

        {fileContributors.length > 1 && (
          <section>
            <h3 className="m-0 mb-1.5 text-[11px] font-semibold uppercase tracking-wider text-muted">
              File-level contributors · {fileContributors.length}
            </h3>
            <ul className="m-0 flex list-none flex-col gap-1 p-0">
              {fileContributors.slice(0, 50).map((e, i) => (
                <li
                  key={i}
                  className="rounded-sm border border-border bg-canvas p-1.5 text-[11px]"
                >
                  <button
                    onClick={() => onSelectNode(e.from)}
                    className="font-mono text-muted hover:text-text"
                    title={`Open ${e.from}`}
                  >
                    {basename(e.from)}
                  </button>
                  <span className="mx-1 text-muted/60">→</span>
                  <button
                    onClick={() => onSelectNode(e.to)}
                    className="font-mono text-muted hover:text-text"
                    title={`Open ${e.to}`}
                  >
                    {basename(e.to)}
                  </button>
                  <span className="ml-2 text-[10px] text-muted/70">
                    ×{e.weight}
                  </span>
                </li>
              ))}
              {fileContributors.length > 50 && (
                <li className="text-[10px] text-muted">
                  + {fileContributors.length - 50} more — drill into one of
                  the endpoints to narrow.
                </li>
              )}
            </ul>
          </section>
        )}

        <section className="flex flex-col gap-1.5">
          <button
            onClick={() => onSelectNode(selectedEdge.from)}
            className="rounded-sm border border-border bg-canvas px-3 py-2 text-left text-[12px] hover:border-text/30"
          >
            <span className="block text-[10px] uppercase tracking-wider text-muted">
              Open source
            </span>
            <span className="font-mono">{selectedEdge.from}</span>
          </button>
          <button
            onClick={() => onSelectNode(selectedEdge.to)}
            className="rounded-sm border border-border bg-canvas px-3 py-2 text-left text-[12px] hover:border-text/30"
          >
            <span className="block text-[10px] uppercase tracking-wider text-muted">
              Open target
            </span>
            <span className="font-mono">{selectedEdge.to}</span>
          </button>
          <button
            onClick={() => {
              const lca = longestCommonAncestor([
                selectedEdge.from,
                selectedEdge.to,
              ]);
              onScope(lca);
              onClose();
            }}
            className="rounded-sm border border-border bg-canvas px-3 py-2 text-left text-[12px] hover:border-text/30"
          >
            <span className="block text-[10px] uppercase tracking-wider text-muted">
              Focus on shared ancestor
            </span>
            <span className="font-mono">
              {longestCommonAncestor([
                selectedEdge.from,
                selectedEdge.to,
              ]) || "root"}
            </span>
          </button>
        </section>
      </div>
    </aside>
  );
}

function basename(p: string): string {
  return p.split("/").filter(Boolean).pop() ?? p;
}

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
  return common.join("/");
}
