import { useEffect, useMemo, useRef, useState } from "react";
import {
  forceSimulation,
  forceManyBody,
  forceLink,
  forceCenter,
  forceCollide,
  type SimulationNodeDatum,
  type SimulationLinkDatum,
} from "d3-force";
import type { NodeEntry, EdgeEntry, ClusterEntry } from "../types";

/**
 * Force-directed cluster view (#278 / PRD §10.3). Lays out the visible
 * node set with d3-force so tightly coupled nodes cluster naturally.
 * Same node set the LSM uses — Force is "the LSM by attraction," not
 * a separate node universe.
 *
 * Soft-caps at 500 nodes; beyond that hairball renders and the
 * simulation chokes anyway, so we render an empty-state card directing
 * the user to drill into a smaller scope.
 *
 * v1 ships pure force layout. Louvain cluster boundaries (precomputed
 * in Rust) are deferred to a follow-up — adding them is purely
 * additive (no contract break) and benefits from being measured
 * separately.
 */
export interface ForceViewProps {
  nodes: NodeEntry[];
  edges: EdgeEntry[];
  /** Pre-computed clusters from the full Data Contract. The view
   *  filters to only those clusters whose members appear in the
   *  current visible node set, then renders boundary tints. */
  clusters?: ClusterEntry[];
  onNodeClick?: (id: string) => void;
}

const NODE_CAP = 500;

interface SimNode extends SimulationNodeDatum {
  id: string;
  layer: string | null;
}

interface SimLink extends SimulationLinkDatum<SimNode> {
  weight: number;
  violates: boolean;
}

export function ForceView({ nodes, edges, clusters, onNodeClick }: ForceViewProps) {
  // Avoid wasted work when the user hits the soft cap.
  if (nodes.length > NODE_CAP) {
    return (
      <div className="m-6 max-w-md rounded-md border border-border bg-canvas p-6 text-[12px]">
        <h3 className="m-0 mb-1 text-[14px] font-semibold">
          Force view paused — too many nodes
        </h3>
        <p className="m-0 text-muted">
          {nodes.length} nodes is past the Force-view soft cap of{" "}
          {NODE_CAP}. A force simulation at this size is a hairball; drill
          into a smaller scope (LSM and Files tabs help) and the Force view
          will paint cleanly.
        </p>
      </div>
    );
  }

  const svgRef = useRef<SVGSVGElement>(null);
  const [tick, setTick] = useState(0);

  // Build sim nodes + links once per (nodes, edges) reference change.
  const sim = useMemo(() => {
    const simNodes: SimNode[] = nodes.map((n) => ({
      id: n.id,
      layer: n.layer,
    }));
    const byId = new Map(simNodes.map((n) => [n.id, n]));
    const simLinks: SimLink[] = edges
      .filter((e) => byId.has(e.from) && byId.has(e.to))
      .map((e) => ({
        source: byId.get(e.from)!,
        target: byId.get(e.to)!,
        weight: e.weight,
        violates: e.violates_rule != null,
      }));
    return { simNodes, simLinks };
  }, [nodes, edges]);

  // Re-run the simulation when inputs change. d3-force mutates the
  // node objects in-place; we trigger React re-renders via `tick`.
  useEffect(() => {
    const simulation = forceSimulation<SimNode>(sim.simNodes)
      .force(
        "link",
        forceLink<SimNode, SimLink>(sim.simLinks)
          .id((d) => d.id)
          .distance(80)
          .strength(0.4),
      )
      .force("charge", forceManyBody().strength(-220))
      .force("center", forceCenter(0, 0))
      .force("collide", forceCollide(18));

    let raf = 0;
    const onTick = () => {
      raf = requestAnimationFrame(() => setTick((t) => t + 1));
    };
    simulation.on("tick", onTick);
    // Stop after enough ticks settle the layout.
    setTimeout(() => simulation.stop(), 4000);
    return () => {
      simulation.stop();
      cancelAnimationFrame(raf);
    };
    // Note: tick is intentionally not in deps — it's the re-render
    // driver, not an input.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [sim]);

  // Resolve cluster boundaries against the currently visible node
  // set. Clusters whose members are containers (e.g.
  // `app/src/.../data`) match visible nodes when those visible nodes
  // are the same path or a descendant. Recomputed every tick so the
  // boundary follows the settling simulation.
  const clusterPositions = (() => {
    if (!clusters || clusters.length === 0) return [];
    // Read `tick` so React's no-unused-vars doesn't trip; the value
    // itself drives nothing — the memo runs every render anyway.
    void tick;
    const byId = new Map(sim.simNodes.map((n) => [n.id, n]));
    return clusters
      .map((c, i) => {
        const points: SimNode[] = [];
        for (const m of c.members) {
          const exact = byId.get(m);
          if (exact) {
            points.push(exact);
            continue;
          }
          for (const n of sim.simNodes) {
            if (n.id.startsWith(m + "/")) points.push(n);
          }
        }
        return { id: c.id, hue: clusterHue(i), points };
      })
      .filter((c) => c.points.length >= 2);
  })();

  // Compute visible bounds for the SVG viewBox.
  const bounds = computeBounds(sim.simNodes);

  return (
    <svg
      ref={svgRef}
      viewBox={`${bounds.minX} ${bounds.minY} ${bounds.width} ${bounds.height}`}
      className="block h-full w-full"
      role="img"
      aria-label={`Force-directed cluster view (${nodes.length} nodes)`}
    >
      {/* Cluster boundaries — rendered first so nodes/edges sit on top. */}
      <g>
        {clusterPositions.map((c) => {
          const c2 = computeClusterCentroid(c.points);
          if (!c2) return null;
          return (
            <circle
              key={c.id}
              cx={c2.cx}
              cy={c2.cy}
              r={c2.radius + 12}
              fill={`hsl(${c.hue}, 70%, 55%, 0.10)`}
              stroke={`hsl(${c.hue}, 70%, 55%, 0.45)`}
              strokeWidth={1}
              strokeDasharray="4 3"
            />
          );
        })}
      </g>
      <g>
        {sim.simLinks.map((l, i) => {
          const s = l.source as SimNode;
          const t = l.target as SimNode;
          if (s.x === undefined || t.x === undefined) return null;
          return (
            <line
              key={i}
              x1={s.x}
              y1={s.y!}
              x2={t.x}
              y2={t.y!}
              strokeWidth={l.violates ? 1.5 : 0.6}
              className={
                l.violates ? "stroke-edge-violation/70" : "stroke-border"
              }
              opacity={0.7}
            />
          );
        })}
      </g>
      <g>
        {sim.simNodes.map((n) => {
          if (n.x === undefined) return null;
          return (
            <g
              key={n.id}
              transform={`translate(${n.x},${n.y})`}
              style={{ cursor: "pointer" }}
              onClick={() => onNodeClick?.(n.id)}
            >
              <circle
                r={6}
                fill={layerFillColor(n.layer)}
                stroke="currentColor"
                strokeWidth={0.5}
                className="text-border"
              />
              <text
                x={9}
                y={3}
                className="text-[9px] fill-text"
                style={{ fontFamily: "ui-sans-serif, system-ui" }}
              >
                {labelOf(n.id)}
              </text>
            </g>
          );
        })}
      </g>
      <text x={bounds.minX + 4} y={bounds.minY + 12} className="text-[10px] fill-muted">
        Force view · {nodes.length} nodes (drag isn't wired; layout is read-only)
        {tick > 0 ? "" : ""}
      </text>
    </svg>
  );
}

/** Distinct hues per cluster index — wraps around the colour wheel. */
function clusterHue(i: number): number {
  return (i * 67) % 360;
}

/** Centroid + radius of a point set so we can draw a single boundary
 *  circle behind the cluster. Convex-hull rendering would look nicer
 *  but adds 2–3 KB of code for a marginal visual improvement; the
 *  bounding circle reads well at a glance. */
function computeClusterCentroid(
  points: SimNode[],
): { cx: number; cy: number; radius: number } | null {
  let cx = 0;
  let cy = 0;
  let count = 0;
  for (const p of points) {
    if (p.x === undefined || p.y === undefined) continue;
    cx += p.x;
    cy += p.y;
    count += 1;
  }
  if (count === 0) return null;
  cx /= count;
  cy /= count;
  let radius = 0;
  for (const p of points) {
    if (p.x === undefined || p.y === undefined) continue;
    const d = Math.hypot(p.x - cx, p.y - cy);
    if (d > radius) radius = d;
  }
  return { cx, cy, radius };
}

function layerFillColor(layer: string | null): string {
  if (!layer) return "rgb(var(--text-muted))";
  if (layer.includes("ui")) return "rgb(var(--accent-ui))";
  if (layer.includes("infra")) return "rgb(var(--accent-infra))";
  if (layer.includes("domain")) return "rgb(var(--accent-domain))";
  return "rgb(var(--text-muted))";
}

function labelOf(id: string): string {
  const segs = id.split("/").filter(Boolean);
  return segs[segs.length - 1] ?? id;
}

function computeBounds(nodes: SimNode[]): {
  minX: number;
  minY: number;
  width: number;
  height: number;
} {
  if (nodes.length === 0)
    return { minX: -200, minY: -100, width: 400, height: 200 };
  let minX = Infinity;
  let minY = Infinity;
  let maxX = -Infinity;
  let maxY = -Infinity;
  for (const n of nodes) {
    if (n.x === undefined || n.y === undefined) continue;
    if (n.x < minX) minX = n.x;
    if (n.y < minY) minY = n.y;
    if (n.x > maxX) maxX = n.x;
    if (n.y > maxY) maxY = n.y;
  }
  if (!isFinite(minX)) return { minX: -200, minY: -100, width: 400, height: 200 };
  const pad = 40;
  return {
    minX: minX - pad,
    minY: minY - pad,
    width: maxX - minX + 2 * pad,
    height: maxY - minY + 2 * pad,
  };
}
