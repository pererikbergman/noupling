import { useMemo } from "react";
import type { DataContract, NodeEntry } from "../types";

export interface MatrixProps {
  data: DataContract;
  onNodeClick?: (id: string) => void;
}

/**
 * NxN dependency matrix view (PRD §10.2).
 *
 * Rows = sources, columns = targets. Cell intensity = log of the edge
 * weight (so a single import doesn't disappear next to a "everything
 * imports this util" cell). Diagonal grey, cycle edges red, violation
 * edges dashed-red.
 *
 * Same scope/filter rules as the LSM — `data.nodes` and `data.edges`
 * arrive pre-narrowed by `CanvasArea`/`App`.
 */
// Safety cap — a 200×200 matrix is 40k cells, well within browser
// budget. Beyond that the unvirtualized table freezes the page, so we
// show an empty state pointing the user at drill-down instead. The
// 200 figure was picked to comfortably handle the noupling self repo
// (~85 immediate children at root) and a typical Android feature
// folder.
const MAX_MATRIX_NODES = 200;

export function Matrix({ data, onNodeClick }: MatrixProps) {
  const layout = useMemo(() => computeMatrixLayout(data), [data]);

  if (layout.nodes.length === 0) {
    return (
      <div className="flex h-full items-center justify-center text-[12px] text-muted">
        No nodes in this scope.
      </div>
    );
  }

  if (layout.nodes.length > MAX_MATRIX_NODES) {
    return (
      <div className="flex h-full items-center justify-center px-8">
        <div className="max-w-md rounded-md border border-border bg-card p-5 text-center">
          <p className="m-0 mb-2 text-[14px] font-semibold text-text">
            Matrix view is bounded
          </p>
          <p className="m-0 text-[12px] leading-relaxed text-muted">
            This scope has{" "}
            <strong className="text-text">{layout.nodes.length}</strong> nodes —
            an {layout.nodes.length}×{layout.nodes.length} matrix would freeze
            the page. Drill into a sub-package (double-click a card on the LSM)
            until the canvas has ≤{MAX_MATRIX_NODES} nodes, then switch back to
            Matrix.
          </p>
        </div>
      </div>
    );
  }

  return (
    <div className="h-full w-full overflow-auto">
      <table
        className="border-collapse text-[10px] font-mono"
        style={{ tableLayout: "fixed" }}
      >
        <thead>
          <tr>
            <th
              className="sticky left-0 top-0 z-20 border-b border-r border-border bg-card p-1.5 text-left text-muted"
              style={{ width: 220 }}
            >
              {layout.nodes.length} × {layout.nodes.length}
            </th>
            {layout.nodes.map((n) => (
              <th
                key={n.id}
                title={n.id}
                className="sticky top-0 z-10 border-b border-r border-border bg-card-header p-0 text-muted"
                style={{ width: 14, height: 80 }}
              >
                <div
                  className="flex items-end justify-center"
                  style={{
                    writingMode: "vertical-rl",
                    transform: "rotate(180deg)",
                    whiteSpace: "nowrap",
                  }}
                >
                  <span className="truncate text-[9px]">{basename(n.id)}</span>
                </div>
              </th>
            ))}
          </tr>
        </thead>
        <tbody>
          {layout.nodes.map((row, i) => (
            <tr key={row.id}>
              <th
                title={row.id}
                onClick={() => onNodeClick?.(row.id)}
                className="sticky left-0 z-10 cursor-pointer truncate border-b border-r border-border bg-card p-1.5 text-left text-[11px] text-text hover:bg-pill hover:text-pill-text"
                style={{ maxWidth: 220, width: 220 }}
              >
                {basename(row.id)}
              </th>
              {layout.nodes.map((col, j) => {
                const cell = layout.cells[i * layout.nodes.length + j];
                return (
                  <td
                    key={col.id}
                    title={
                      cell
                        ? `${row.id} → ${col.id} (×${cell.weight})${cell.isCycle ? " · cycle" : ""}${cell.violatesRule ? " · violation: " + cell.violatesRule : ""}`
                        : `${row.id} → ${col.id}`
                    }
                    className="border-b border-r border-border"
                    style={{
                      width: 14,
                      height: 14,
                      background: cellFill(cell, i === j),
                    }}
                  />
                );
              })}
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

interface MatrixCell {
  weight: number;
  isCycle: boolean;
  violatesRule: string | null;
}

interface MatrixLayout {
  nodes: NodeEntry[];
  cells: Array<MatrixCell | null>;
}

function computeMatrixLayout(data: DataContract): MatrixLayout {
  // Sort: by layer (in source-declared order), then alphabetically. Files
  // first inside each layer so packages don't push them apart.
  const layerOrder = new Map<string, number>();
  data.layers.forEach((l, i) => layerOrder.set(l.name, i));
  const nodes = [...data.nodes].sort((a, b) => {
    const al = a.layer ? (layerOrder.get(a.layer) ?? 999) : 999;
    const bl = b.layer ? (layerOrder.get(b.layer) ?? 999) : 999;
    if (al !== bl) return al - bl;
    const ak = a.kind === "file" ? 0 : 1;
    const bk = b.kind === "file" ? 0 : 1;
    if (ak !== bk) return ak - bk;
    return a.id.localeCompare(b.id);
  });

  const idIndex = new Map(nodes.map((n, i) => [n.id, i]));
  const cells: Array<MatrixCell | null> = new Array(nodes.length * nodes.length).fill(null);
  const cycleEdges = new Set<string>();
  for (const c of data.cycles) {
    for (let k = 0; k < c.members.length; k++) {
      const from = c.members[k];
      const to = c.members[(k + 1) % c.members.length];
      cycleEdges.add(`${from}→${to}`);
    }
  }
  for (const e of data.edges) {
    const i = idIndex.get(e.from);
    const j = idIndex.get(e.to);
    if (i === undefined || j === undefined) continue;
    cells[i * nodes.length + j] = {
      weight: e.weight,
      isCycle: cycleEdges.has(`${e.from}→${e.to}`),
      violatesRule: e.violates_rule,
    };
  }
  return { nodes, cells };
}

function cellFill(cell: MatrixCell | null, isDiagonal: boolean): string {
  if (isDiagonal) return "rgba(var(--border) / 0.4)";
  if (!cell) return "transparent";
  if (cell.violatesRule) return "rgb(var(--edge-violation) / 0.7)";
  if (cell.isCycle) return "rgb(var(--edge-cycle) / 0.7)";
  // log-scale intensity 0.15 → 0.85 across weights 1 → 32.
  const alpha = Math.min(0.85, 0.15 + Math.log2(cell.weight + 1) * 0.18);
  return `rgb(var(--accent-domain) / ${alpha})`;
}

function basename(p: string): string {
  return p.split("/").filter(Boolean).pop() ?? p;
}
