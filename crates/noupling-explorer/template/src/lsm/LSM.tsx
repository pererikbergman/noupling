import { useEffect, useMemo, useState } from "react";
import type { DataContract } from "../types";
import type { EdgeAccent, HighlightPolicy } from "../state/highlightPolicy";
import { computeLSMLayout, type LayerBand, type PositionedEdge, type PositionedNode } from "./layout";

export interface LSMProps {
  data: DataContract;
  onNodeClick?: (id: string) => void;
  onNodeDoubleClick?: (id: string) => void;
  /** Canvas-wide highlight policy (#318). Owns the precedence rule
   *  between path / min-cut / violation / cycle / selected accents
   *  plus the per-node cycle badge counts. */
  highlight: HighlightPolicy;
  /** Click handler for edges. Invoked with the edge's from/to ids. */
  onEdgeClick?: (from: string, to: string) => void;
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
export function LSM({
  data,
  onNodeClick,
  onNodeDoubleClick,
  highlight,
  onEdgeClick,
}: LSMProps) {
  const layerOverlay = highlight.layerOverlay;
  const layout = useMemo(() => computeLSMLayout(data), [data]);
  const [hovered, setHovered] = useState<string | null>(null);
  // A drill or filter replaces the cards under the pointer; the old hover
  // must not keep dimming the new ones (#405).
  useEffect(() => setHovered(null), [data]);

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
        <LayerBandRect
          key={`band-${band.index}-${band.name}`}
          band={band}
          width={layout.width}
          overlay={layerOverlay}
        />
      ))}

      {/* Edges */}
      <g>
        {layout.edges.map((e) => {
          const accent = highlight.edgeAccent(
            e.from,
            e.to,
            e.isViolation,
            e.isCycle,
          );
          return (
            <EdgePath
              key={`${e.from}->${e.to}`}
              edge={e}
              dimmed={directDeps !== null && !(directDeps.has(e.from) && directDeps.has(e.to))}
              accent={accent}
              onClick={
                onEdgeClick ? () => onEdgeClick(e.from, e.to) : undefined
              }
            />
          );
        })}
      </g>

      {/* Nodes */}
      <g>
        {layout.nodes.map((n) => (
          <NodeCard
            key={n.id}
            node={n}
            dimmed={directDeps !== null && !directDeps.has(n.id)}
            cycleBadgeCount={highlight.cycleBadgeCount(n.id)}
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

function LayerBandRect({
  band,
  width,
  overlay,
}: {
  band: LayerBand;
  width: number;
  overlay: boolean;
}) {
  // Cool blue/green for clean layers, warming toward red as violation rate climbs.
  // Stays inside the WCAG-AA palette declared in styles.css.
  const cleanFill = "rgba(20, 184, 166, 0.07)";
  const warmFill = `rgba(255, 69, 58, ${Math.min(0.16, 0.06 + band.violationRate * 0.16)})`;
  const healthFill = band.violationRate > 0 ? warmFill : cleanFill;
  const identityFill = overlay ? layerOverlayTint(band.name) : "none";
  return (
    <g>
      {overlay && (
        <rect x={0} y={band.y} width={width} height={band.height} fill={identityFill} />
      )}
      <rect x={0} y={band.y} width={width} height={band.height} fill={healthFill} />
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
        {band.instability !== null ? ` · I=${band.instability.toFixed(2)}` : ""}
      </text>
    </g>
  );
}

function layerOverlayTint(name: string): string {
  // Pick a deterministic muted tint per layer name so user can tell tiers
  // apart on the overlay view independent of health.
  const palette = [
    "rgba(167, 139, 250, 0.10)",
    "rgba(20, 184, 166, 0.10)",
    "rgba(249, 115, 22, 0.10)",
    "rgba(96, 165, 250, 0.10)",
    "rgba(248, 113, 113, 0.10)",
    "rgba(244, 114, 182, 0.10)",
  ];
  let h = 0;
  for (const ch of name) h = (h * 31 + ch.charCodeAt(0)) | 0;
  return palette[Math.abs(h) % palette.length];
}

function EdgePath({
  edge,
  dimmed,
  accent,
  onClick,
}: {
  edge: PositionedEdge;
  dimmed: boolean;
  accent: EdgeAccent;
  onClick?: () => void;
}) {
  const stroke = ACCENT_STROKE[accent];
  const dash = accent === "minCut" ? "4 4" : accent === "violation" ? "6 4" : "0";
  const isPriority =
    accent === "selected" || accent === "path" || accent === "minCut";
  const opacity = dimmed
    ? 0.18
    : isPriority
      ? 1
      : accent === "cycle" || accent === "violation"
        ? 0.95
        : 0.55;
  const baseWidth = 1 + Math.min(edge.weight, 4) * 0.4;
  const strokeWidth = isPriority ? baseWidth + 1.5 : baseWidth;
  const d = edgePathD(edge);
  return (
    <g
      style={onClick ? { cursor: "pointer" } : undefined}
      onClick={onClick}
    >
      {/* Visible edge */}
      <path
        d={d}
        fill="none"
        stroke={stroke}
        strokeWidth={strokeWidth}
        strokeDasharray={dash}
        opacity={opacity}
        markerEnd={`url(#${accent === "cycle" ? "lsm-arrow-cycle" : "lsm-arrow"})`}
        data-edge={`${edge.from}→${edge.to}`}
        data-accent={accent}
      >
        {edge.isViolation && edge.violationMessage ? (
          <title>{edge.violationMessage}</title>
        ) : (
          <title>
            {edge.from} → {edge.to} · click for details
          </title>
        )}
      </path>
      {/* Fat invisible hit area — thin SVG paths are awkward to click;
          a 12-px wide transparent overlay catches the click reliably. */}
      {onClick && (
        <path
          d={d}
          fill="none"
          stroke="transparent"
          strokeWidth={12}
          pointerEvents="stroke"
        />
      )}
    </g>
  );
}

/**
 * SVG path for a routed edge (#398). Adjacent-tier edges keep the
 * S-curve; lane and arc routes follow their polyline with rounded
 * corners so the eye can trace a long edge past the nodes it skips.
 */
function edgePathD(edge: PositionedEdge): string {
  const pts = edge.points;
  if (edge.kind === "direct" || pts.length < 3) {
    const midY = (edge.y1 + edge.y2) / 2;
    return `M ${edge.x1} ${edge.y1} C ${edge.x1} ${midY}, ${edge.x2} ${midY}, ${edge.x2} ${edge.y2}`;
  }
  const r = 10;
  let d = `M ${pts[0].x} ${pts[0].y}`;
  for (let i = 1; i < pts.length - 1; i++) {
    const prev = pts[i - 1];
    const cur = pts[i];
    const next = pts[i + 1];
    // Stop `r` short of the corner, then quadratic-curve through it.
    const inLen = Math.hypot(cur.x - prev.x, cur.y - prev.y);
    const outLen = Math.hypot(next.x - cur.x, next.y - cur.y);
    const ri = Math.min(r, inLen / 2);
    const ro = Math.min(r, outLen / 2);
    const ax = cur.x - ((cur.x - prev.x) / (inLen || 1)) * ri;
    const ay = cur.y - ((cur.y - prev.y) / (inLen || 1)) * ri;
    const bx = cur.x + ((next.x - cur.x) / (outLen || 1)) * ro;
    const by = cur.y + ((next.y - cur.y) / (outLen || 1)) * ro;
    d += ` L ${ax} ${ay} Q ${cur.x} ${cur.y} ${bx} ${by}`;
  }
  const last = pts[pts.length - 1];
  d += ` L ${last.x} ${last.y}`;
  return d;
}

const ACCENT_STROKE: Record<EdgeAccent, string> = {
  selected: "rgb(var(--accent-domain))",
  path: "rgb(var(--accent-ui))",
  minCut: "rgb(var(--accent-infra))",
  violation: "rgb(var(--edge-violation))",
  cycle: "rgb(var(--edge-cycle))",
  default: "rgb(var(--text-muted))",
};

function NodeCard({
  node,
  dimmed,
  cycleBadgeCount,
  onMouseEnter,
  onMouseLeave,
  onClick,
  onDoubleClick,
}: {
  node: PositionedNode;
  dimmed: boolean;
  cycleBadgeCount: number;
  onMouseEnter: () => void;
  onMouseLeave: () => void;
  onClick: () => void;
  onDoubleClick: () => void;
}) {
  const stripeColor = layerAccentColor(node.layer);
  const baseOpacity = dimmed ? 0.35 : 1;
  const ariaLabel = `${node.label} (${node.layer ?? "unlayered"})`;
  return (
    <g
      role="button"
      tabIndex={0}
      aria-label={ariaLabel}
      transform={`translate(${node.x},${node.y})`}
      opacity={baseOpacity}
      style={{ cursor: "pointer", transition: "opacity 120ms", outline: "none" }}
      onMouseEnter={onMouseEnter}
      onMouseLeave={onMouseLeave}
      onClick={onClick}
      onDoubleClick={onDoubleClick}
      onFocus={onMouseEnter}
      onBlur={onMouseLeave}
      onKeyDown={(e) => {
        if (e.key === "Enter" || e.key === " ") {
          e.preventDefault();
          onClick();
        } else if (e.key === "d" || e.key === "D") {
          e.preventDefault();
          onDoubleClick();
        }
      }}
    >
      {/* Non-file kinds get a thicker stripe, slightly darker card fill,
          and a more prominent kind badge so drillable nodes stand out
          from leaf files. */}
      <rect
        x={0}
        y={0}
        width={node.width}
        height={node.height}
        rx={12}
        ry={12}
        fill={node.kind === "file" ? "rgb(var(--card))" : "rgb(var(--card-header))"}
        stroke="rgb(var(--border))"
        strokeWidth={node.kind === "file" ? 1 : 1.5}
      />
      <rect
        x={0}
        y={0}
        width={node.kind === "file" ? 4 : 6}
        height={node.height}
        fill={stripeColor}
        rx={2}
        ry={2}
      />
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
        {node.kind === "file"
          ? "file"
          : node.kind === "package"
            ? `▸ ${node.fileCount} files`
            : `▸ container`}
      </text>
      <text x={14} y={46} fill="rgb(var(--text))" fontSize={14} fontWeight={600}>
        {truncate(node.label, 22)}
      </text>
      <text x={14} y={64} fill="rgb(var(--text-muted))" fontSize={11}>
        {truncate(node.sublabel, 28)}
      </text>
      {cycleBadgeCount > 0 && (
        <g>
          <circle
            cx={node.width - 12}
            cy={node.height - 12}
            r={9}
            fill="rgb(var(--edge-cycle))"
            opacity={0.85}
          />
          <text
            x={node.width - 12}
            y={node.height - 9}
            textAnchor="middle"
            fill="#fff"
            fontSize={9}
            fontWeight={700}
            fontFamily="ui-monospace, monospace"
          >
            {cycleBadgeCount}
          </text>
        </g>
      )}
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
