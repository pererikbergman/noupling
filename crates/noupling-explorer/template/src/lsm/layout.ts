/**
 * LSM layout — pure function that turns the Data Contract into the
 * absolute geometry the SVG renderer paints. Separated from the
 * component so the layout is unit-testable and deterministic.
 */
import type { DataContract, EdgeEntry, LayerEntry, NodeEntry } from "../types";

export interface PositionedNode {
  id: string;
  label: string;
  sublabel: string;
  kind: NodeEntry["kind"];
  layer: string | null;
  layerIndex: number; // tier index; -1 for unlayered
  x: number;
  y: number;
  width: number;
  height: number;
  fileCount: number;
}

export interface PositionedEdge {
  from: string;
  to: string;
  weight: number;
  isCycle: boolean;
  isViolation: boolean;
  violationMessage: string | null;
  // Endpoint coordinates (centered on the source's bottom and the target's top).
  x1: number;
  y1: number;
  x2: number;
  y2: number;
}

export interface LayerBand {
  name: string;
  index: number;
  y: number;
  height: number;
  violationRate: number;
  fileCount: number;
  afferent: number;
  efferent: number;
  instability: number | null;
}

export interface LSMLayout {
  width: number;
  height: number;
  nodes: PositionedNode[];
  edges: PositionedEdge[];
  bands: LayerBand[];
}

const TIER_HEIGHT = 200;
const TIER_PADDING_TOP = 40;
const NODE_LEAF_W = 170;
const NODE_LEAF_H = 78;
const NODE_PADDING = 32;
const SIDE_PADDING = 32;
const UNLAYERED_LABEL = "(unlayered)";

/**
 * Compute the absolute geometry for the LSM.
 *
 * Tiers are derived from `data.layers` ordered by `index`. Unlayered file
 * nodes get their own tier at the bottom. Container nodes are excluded
 * from the LSM proper (they're the navigation surface for drill-down,
 * not rendered as graph atoms). Package nodes appear at the head of their
 * layer tier when there are file nodes below them.
 */
export function computeLSMLayout(data: DataContract): LSMLayout {
  // 1. Group file-kind nodes by layer. Containers and packages are
  // navigation surfaces for drill-down (#234), not atoms of the LSM —
  // showing them here would clutter the tier with parent-of-parent
  // groupings that aren't reachable in v1 anyway.
  const layeredNodes = new Map<number, NodeEntry[]>();
  const unlayered: NodeEntry[] = [];
  for (const n of data.nodes) {
    if (n.kind !== "file") continue;
    const layerIdx = findLayerIndex(n.layer, data.layers);
    if (layerIdx === -1) {
      unlayered.push(n);
    } else {
      if (!layeredNodes.has(layerIdx)) layeredNodes.set(layerIdx, []);
      layeredNodes.get(layerIdx)!.push(n);
    }
  }

  // 2. Build the tier list (top to bottom by index).
  const sortedLayers = [...data.layers].sort((a, b) => a.index - b.index);
  const tiers: Array<{ layer: LayerEntry | null; nodes: NodeEntry[] }> = sortedLayers.map((l) => ({
    layer: l,
    nodes: (layeredNodes.get(l.index) ?? []).slice().sort((a, b) => a.id.localeCompare(b.id)),
  }));
  if (unlayered.length > 0) {
    tiers.push({
      layer: null,
      nodes: unlayered.slice().sort((a, b) => a.id.localeCompare(b.id)),
    });
  }

  // 3. Compute SVG width: widest tier.
  const widestTier = Math.max(
    ...tiers.map((t) => t.nodes.length * (NODE_LEAF_W + NODE_PADDING) + NODE_PADDING),
    600,
  );
  const width = widestTier + SIDE_PADDING * 2;
  const height = tiers.length * TIER_HEIGHT;

  // 4. Position nodes.
  const nodes: PositionedNode[] = [];
  const bands: LayerBand[] = [];
  tiers.forEach((tier, tierIdx) => {
    const y0 = tierIdx * TIER_HEIGHT;
    const layer = tier.layer;
    bands.push({
      name: layer?.name ?? UNLAYERED_LABEL,
      index: layer?.index ?? -1,
      y: y0,
      height: TIER_HEIGHT,
      violationRate: layer ? violationRateFor(layer, data) : 0,
      fileCount: layer?.file_count ?? tier.nodes.length,
      afferent: layer?.afferent ?? 0,
      efferent: layer?.efferent ?? 0,
      instability: layer?.instability ?? null,
    });

    // Left-align nodes within each tier so the first N nodes of every
    // tier are immediately visible. Wider tiers extend off to the
    // right and the canvas scrolls.
    const xOffset = SIDE_PADDING;
    tier.nodes.forEach((n, i) => {
      const nodeW = nodeWidthFor(n);
      const nodeH = NODE_LEAF_H;
      nodes.push({
        id: n.id,
        label: basename(n.id),
        sublabel: n.id,
        kind: n.kind,
        layer: n.layer,
        layerIndex: layer?.index ?? -1,
        x: xOffset + i * (NODE_LEAF_W + NODE_PADDING) + (NODE_LEAF_W - nodeW) / 2,
        y: y0 + TIER_PADDING_TOP,
        width: nodeW,
        height: nodeH,
        fileCount: typeof n.metrics.file_count === "number" ? n.metrics.file_count : 1,
      });
    });
  });

  // 5. Position edges between nodes (skip edges that touch a missing node).
  const byId = new Map(nodes.map((n) => [n.id, n] as const));
  const cycleEdgeSet = buildCycleEdgeSet(data);
  const edges: PositionedEdge[] = [];
  for (const e of data.edges) {
    const a = byId.get(e.from);
    const b = byId.get(e.to);
    if (!a || !b) continue;
    edges.push({
      from: e.from,
      to: e.to,
      weight: e.weight,
      isViolation: e.violates_rule !== null,
      violationMessage: e.violates_rule,
      isCycle: cycleEdgeSet.has(edgeKey(e)),
      x1: a.x + a.width / 2,
      y1: a.y + a.height,
      x2: b.x + b.width / 2,
      y2: b.y,
    });
  }

  return { width, height, nodes, edges, bands };
}

function findLayerIndex(layer: string | null, layers: LayerEntry[]): number {
  if (!layer) return -1;
  const found = layers.find((l) => l.name === layer);
  return found?.index ?? -1;
}

/**
 * Compute the violation rate (0..1) for a layer. Higher = more red tint.
 *
 * For now: count violations whose source layer matches this layer, then
 * normalise by total layer file count. The exact formula is deliberately
 * conservative — a layer with even one violation gets a noticeable tint.
 */
function violationRateFor(layer: LayerEntry, data: DataContract): number {
  if (layer.file_count === 0) return 0;
  // Count violations whose edge.from path falls inside this layer.
  const involved = data.violations.filter((v) => {
    const fromNode = data.nodes.find((n) => n.id === v.edge.from);
    return fromNode?.layer === layer.name;
  }).length;
  return Math.min(1, involved / Math.max(1, layer.file_count / 5));
}

function nodeWidthFor(n: NodeEntry): number {
  return n.kind === "package" ? NODE_LEAF_W + 30 : NODE_LEAF_W;
}

function basename(id: string): string {
  return id.split("/").filter(Boolean).pop() ?? id;
}

function buildCycleEdgeSet(data: DataContract): Set<string> {
  const cycleNodes = new Set<string>(data.cycles.flatMap((c) => c.members));
  const out = new Set<string>();
  for (const e of data.edges) {
    if (cycleNodes.has(e.from) && cycleNodes.has(e.to)) {
      out.add(edgeKey(e));
    }
  }
  return out;
}

function edgeKey(e: Pick<EdgeEntry, "from" | "to">): string {
  return `${e.from}${e.to}`;
}
