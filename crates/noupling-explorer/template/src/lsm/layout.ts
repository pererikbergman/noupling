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
  // Render every node kind — files, packages, containers. The caller is
  // responsible for narrowing `data.nodes` to whatever should be on the
  // canvas at the current scope (#255).
  const tiers = buildTiers(data.nodes, data.layers);

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
      name: layer?.name ?? tier.syntheticName ?? UNLAYERED_LABEL,
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

interface Tier {
  layer: LayerEntry | null;
  /** Synthetic tier label when `layer` is null (e.g. derived from a path segment). */
  syntheticName?: string;
  nodes: NodeEntry[];
}

/**
 * Build the ordered tier list the LSM renders.
 *
 * Three cases:
 *  1. **Layered codebase** — `data.layers` is non-empty AND at least one
 *     file node matches a layer. Group nodes by layer index; unlayered
 *     remainder lands in a trailing tier.
 *  2. **Fully unlayered** — no `data.layers` entries, OR no file matches
 *     any layer (the user shipped a settings.json whose patterns don't
 *     fit this codebase). Derive synthetic tiers from each file's last
 *     directory segment (`a/b/c/file.kt` → tier `c`). Sorted alpha so
 *     siblings cluster.
 *  3. **Mixed** — some layered, some not. Layered tiers first, then a
 *     trailing tier per directory segment for the rest.
 */
function buildTiers(nodes: NodeEntry[], layers: LayerEntry[]): Tier[] {
  const layeredByIndex = new Map<number, NodeEntry[]>();
  const unlayered: NodeEntry[] = [];
  for (const n of nodes) {
    const i = findLayerIndex(n.layer, layers);
    if (i === -1) unlayered.push(n);
    else {
      if (!layeredByIndex.has(i)) layeredByIndex.set(i, []);
      layeredByIndex.get(i)!.push(n);
    }
  }

  const tiers: Tier[] = [];
  const sortedLayers = [...layers].sort((a, b) => a.index - b.index);
  for (const l of sortedLayers) {
    const ns = layeredByIndex.get(l.index) ?? [];
    if (ns.length === 0) continue; // skip empty configured layers — they add visual noise
    tiers.push({ layer: l, nodes: ns.slice().sort((a, b) => a.id.localeCompare(b.id)) });
  }

  // Synthetic tiers from the unlayered remainder, grouped by last dir segment.
  // Falls back to a single "(unlayered)" tier when the synthetic grouping
  // doesn't help (1 group, or every file in the same dir as its only sibling).
  if (unlayered.length > 0) {
    const bySegment = new Map<string, NodeEntry[]>();
    for (const n of unlayered) {
      const seg = lastDirSegment(n.id) || UNLAYERED_LABEL;
      if (!bySegment.has(seg)) bySegment.set(seg, []);
      bySegment.get(seg)!.push(n);
    }
    const useSynthetic = bySegment.size >= 2;
    if (useSynthetic) {
      const segmentNames = [...bySegment.keys()].sort();
      for (const seg of segmentNames) {
        tiers.push({
          layer: null,
          syntheticName: seg,
          nodes: bySegment
            .get(seg)!
            .slice()
            .sort((a, b) => a.id.localeCompare(b.id)),
        });
      }
    } else {
      tiers.push({
        layer: null,
        syntheticName: UNLAYERED_LABEL,
        nodes: unlayered.slice().sort((a, b) => a.id.localeCompare(b.id)),
      });
    }
  }

  // Hard cap — beyond this the LSM is hard to read; better to bucket
  // overflow into one trailing tier than render 60 thin rows.
  const MAX_TIERS = 16;
  if (tiers.length > MAX_TIERS) {
    const kept = tiers.slice(0, MAX_TIERS - 1);
    const overflowNodes = tiers
      .slice(MAX_TIERS - 1)
      .flatMap((t) => t.nodes)
      .sort((a, b) => a.id.localeCompare(b.id));
    kept.push({
      layer: null,
      syntheticName: `… +${tiers.length - MAX_TIERS + 1} more`,
      nodes: overflowNodes,
    });
    return kept;
  }
  return tiers;
}

function lastDirSegment(path: string): string {
  const parts = path.split("/").filter(Boolean);
  if (parts.length < 2) return "";
  return parts[parts.length - 2];
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
