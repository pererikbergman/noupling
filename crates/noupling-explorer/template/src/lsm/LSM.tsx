import { useMemo, useState } from "react";
import type { DataContract } from "../types";
import { computeLSMLayout, type LayerBand, type PositionedEdge, type PositionedNode } from "./layout";

export interface LSMProps {
  data: DataContract;
  onNodeClick?: (id: string) => void;
  onNodeDoubleClick?: (id: string) => void;
}

/**
 * The Layered Structure Map — the headline view of the Explorer.
 *
 * Renders nodes in tiers by their `layer` (top to bottom by layer index),
 * with cross-layer edges drawn between them. Layer bands are tinted by
 * violation rate. Cycle edges are red; rule violations are red dashed.
 *
 * v1 of #233: read-only. Drill-down is #234, details panel is #235.
 */
export function LSM({ data, onNodeClick, onNodeDoubleClick }: LSMProps) {
  const layout = useMemo(() => computeLSMLayout(data), [data]);
  const [hovered, setHovered] = useState<string | null>(null);

  const directDeps = useMemo(() => {
    if (!hovered) return null;
    const out = new Set<string>([hovered]);
    for (const e of layout.edges) {
      if (e.from === hovered) out.add(e.to);
      if (e.to === hovered) out.add(e.from);
    }
    return out;
  }, [hovered, layout.edges]);

  return (
    <svg
      width={layout.width}
      height={layout.height}
      viewBox={`0 0 ${layout.width} ${layout.height}`}
      role="img"
      aria-label="Layered Structure Map"
      className="block"
    >
      <defs>
        <marker
          id="lsm-arrow"
          viewBox="0 0 10 10"
          refX="9"
          refY="5"
          markerWidth="6"
          markerHeight="6"
          orient="auto"
        >
          <path d="M0,0 L10,5 L0,10 z" fill="rgb(var(--text-muted))" />
        </marker>
        <marker
          id="lsm-arrow-cycle"
          viewBox="0 0 10 10"
          refX="9"
          refY="5"
          markerWidth="7"
          markerHeight="7"
          orient="auto"
        >
          <path d="M0,0 L10,5 L0,10 z" fill="rgb(var(--edge-cycle))" />
        </marker>
      </defs>

      {/* Layer health bands */}
      {layout.bands.map((band) => (
        <LayerBandRect key={`band-${band.index}-${band.name}`} band={band} width={layout.width} />
      ))}

      {/* Edges */}
      <g>
        {layout.edges.map((e) => (
          <EdgePath
            key={`${e.from}->${e.to}`}
            edge={e}
            dimmed={directDeps !== null && !(directDeps.has(e.from) && directDeps.has(e.to))}
          />
        ))}
      </g>

      {/* Nodes */}
      <g>
        {layout.nodes.map((n) => (
          <NodeCard
            key={n.id}
            node={n}
            dimmed={directDeps !== null && !directDeps.has(n.id)}
            onMouseEnter={() => setHovered(n.id)}
            onMouseLeave={() => setHovered((h) => (h === n.id ? null : h))}
            onClick={() => onNodeClick?.(n.id)}
            onDoubleClick={() => onNodeDoubleClick?.(n.id)}
          />
        ))}
      </g>
    </svg>
  );
}

function LayerBandRect({ band, width }: { band: LayerBand; width: number }) {
  // Cool blue/green for clean layers, warming toward red as violation rate climbs.
  // Stays inside the WCAG-AA palette declared in styles.css.
  const cleanFill = "rgba(20, 184, 166, 0.07)";
  const warmFill = `rgba(255, 69, 58, ${Math.min(0.16, 0.06 + band.violationRate * 0.16)})`;
  const fill = band.violationRate > 0 ? warmFill : cleanFill;
  return (
    <g>
      <rect x={0} y={band.y} width={width} height={band.height} fill={fill} />
      <text
        x={20}
        y={band.y + 22}
        fill="rgb(var(--text-muted))"
        fontSize={10}
        letterSpacing={2}
        fontFamily="ui-monospace, monospace"
        style={{ textTransform: "uppercase" }}
      >
        {band.name.toUpperCase()} · {band.fileCount}f{" "}
        {band.violationRate > 0 ? "· violations" : "· clean"}
        {band.instability !== null
          ? ` · I=${band.instability.toFixed(2)}`
          : ""}
      </text>
    </g>
  );
}

function EdgePath({ edge, dimmed }: { edge: PositionedEdge; dimmed: boolean }) {
  const stroke = edge.isViolation
    ? "rgb(var(--edge-violation))"
    : edge.isCycle
      ? "rgb(var(--edge-cycle))"
      : "rgb(var(--text-muted))";
  const dash = edge.isViolation ? "6 4" : "0";
  const opacity = dimmed ? 0.18 : edge.isCycle || edge.isViolation ? 0.95 : 0.55;
  const strokeWidth = 1 + Math.min(edge.weight, 4) * 0.4;
  const midY = (edge.y1 + edge.y2) / 2;
  return (
    <path
      d={`M ${edge.x1} ${edge.y1} C ${edge.x1} ${midY}, ${edge.x2} ${midY}, ${edge.x2} ${edge.y2}`}
      fill="none"
      stroke={stroke}
      strokeWidth={strokeWidth}
      strokeDasharray={dash}
      opacity={opacity}
      markerEnd={`url(#${edge.isCycle ? "lsm-arrow-cycle" : "lsm-arrow"})`}
    >
      {edge.isViolation && edge.violationMessage ? (
        <title>{edge.violationMessage}</title>
      ) : null}
    </path>
  );
}

function NodeCard({
  node,
  dimmed,
  onMouseEnter,
  onMouseLeave,
  onClick,
  onDoubleClick,
}: {
  node: PositionedNode;
  dimmed: boolean;
  onMouseEnter: () => void;
  onMouseLeave: () => void;
  onClick: () => void;
  onDoubleClick: () => void;
}) {
  const stripeColor = layerAccentColor(node.layer);
  const baseOpacity = dimmed ? 0.35 : 1;
  return (
    <g
      transform={`translate(${node.x},${node.y})`}
      opacity={baseOpacity}
      style={{ cursor: "pointer", transition: "opacity 120ms" }}
      onMouseEnter={onMouseEnter}
      onMouseLeave={onMouseLeave}
      onClick={onClick}
      onDoubleClick={onDoubleClick}
    >
      <rect
        x={0}
        y={0}
        width={node.width}
        height={node.height}
        rx={12}
        ry={12}
        fill="rgb(var(--card))"
        stroke="rgb(var(--border))"
      />
      <rect x={0} y={0} width={4} height={node.height} fill={stripeColor} rx={2} ry={2} />
      <text
        x={14}
        y={22}
        fill="rgb(var(--text-muted))"
        fontSize={9}
        letterSpacing={1.5}
        fontFamily="ui-monospace, monospace"
        style={{ textTransform: "uppercase" }}
      >
        {(node.layer ?? "unlayered").toUpperCase()}
      </text>
      <text
        x={node.width - 14}
        y={22}
        textAnchor="end"
        fill="rgb(var(--text-muted))"
        fontSize={9}
        fontFamily="ui-monospace, monospace"
      >
        {node.kind === "package" ? `${node.fileCount} files` : "file"}
      </text>
      <text x={14} y={46} fill="rgb(var(--text))" fontSize={14} fontWeight={600}>
        {truncate(node.label, 22)}
      </text>
      <text x={14} y={64} fill="rgb(var(--text-muted))" fontSize={11}>
        {truncate(node.sublabel, 28)}
      </text>
    </g>
  );
}

function layerAccentColor(layer: string | null): string {
  if (!layer) return "rgb(var(--text-muted))";
  if (layer.includes("ui")) return "rgb(var(--accent-ui))";
  if (layer.includes("infra")) return "rgb(var(--accent-infra))";
  if (layer.includes("domain")) return "rgb(var(--accent-domain))";
  // Cycle through accents for arbitrary layer names.
  const palette = [
    "rgb(var(--accent-ui))",
    "rgb(var(--accent-domain))",
    "rgb(var(--accent-infra))",
  ];
  let h = 0;
  for (const ch of layer) h = (h * 31 + ch.charCodeAt(0)) | 0;
  return palette[Math.abs(h) % palette.length];
}

function truncate(s: string, max: number): string {
  return s.length <= max ? s : s.slice(0, max - 1) + "…";
}
