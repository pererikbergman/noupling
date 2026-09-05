/**
 * LSM layout — pure function that turns the Data Contract into the
 * absolute geometry the SVG renderer paints. Separated from the
 * component so the layout is unit-testable and deterministic.
 */
import type { DataContract, EdgeEntry, LayerEntry, NodeEntry } from "../types";
// Explicit extension: the unit tests run this file under node --experimental-strip-types.
import { displayLabel } from "../labels.ts";

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

export interface Point {
  x: number;
  y: number;
}

/**
 * How an edge is routed (#398):
 * - `direct`: source and target sit in adjacent tiers — a curve through
 *   the node-free gap between them;
 * - `lane`: the edge spans more than one tier (or points upward), so it
 *   leaves the source, runs down a vertical lane to the right of the
 *   nodes, and comes back in to the target — never through a node;
 * - `arc`: both ends are in the same tier — an arc below the nodes.
 */
export type EdgeRoute = "direct" | "lane" | "arc";

export interface PositionedEdge {
  from: string;
  to: string;
  weight: number;
  isCycle: boolean;
  isViolation: boolean;
  violationMessage: string | null;
  kind: EdgeRoute;
  /** Polyline the renderer smooths; first point leaves the source, last
   *  point lands on the target (top edge for downward routes, bottom
   *  edge for arcs and upward lanes). */
  points: Point[];
  // Endpoint coordinates, kept for callers that only need the ends.
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

const TIER_PADDING_TOP = 36;
/** Room under the last row of a tier for edge stubs and same-tier arcs. */
const TIER_PADDING_BOTTOM = 62;
/** Cards per row before a tier wraps. */
const MAX_PER_ROW = 6;
const ROW_GAP = 40;
const NODE_LEAF_W = 170;
const NODE_LEAF_H = 78;
const NODE_PADDING = 32;
const SIDE_PADDING = 32;
/** Vertical clearance an edge keeps from a node before turning. */
const EDGE_STUB = 16;
/** Gap between the right-most node and the first lane; step per lane. */
const LANE_GUTTER = 28;
const LANE_STEP = 12;
/** First same-tier arc dips this far below the nodes; each further arc a bit more. */
const ARC_DIP = 26;
const ARC_STEP = 10;
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

  // 3. Rows: a tier wider than MAX_PER_ROW cards wraps onto further rows
  //    so a busy directory grows downward and stays legible when the
  //    canvas fits it, instead of becoming a 30%-zoom ribbon (#399).
  const rowsOf = (t: Tier) => Math.max(1, Math.ceil(t.nodes.length / MAX_PER_ROW));
  const tierHeightOf = (t: Tier) =>
    TIER_PADDING_TOP + rowsOf(t) * NODE_LEAF_H + (rowsOf(t) - 1) * ROW_GAP + TIER_PADDING_BOTTOM;
  const widestRow = Math.max(
    ...tiers.map((t) => Math.min(t.nodes.length, MAX_PER_ROW) * (NODE_LEAF_W + NODE_PADDING) + NODE_PADDING),
    600,
  );
  const width = widestRow + SIDE_PADDING * 2;
  const height = tiers.reduce((acc, t) => acc + tierHeightOf(t), 0);

  // 4. Position nodes.
  const nodes: PositionedNode[] = [];
  const bands: LayerBand[] = [];
  let y0 = 0;
  tiers.forEach((tier) => {
    const layer = tier.layer;
    const tierHeight = tierHeightOf(tier);
    const filesInTier = tier.nodes.reduce(
      (acc, n) => acc + (typeof n.metrics.file_count === "number" ? n.metrics.file_count : 1),
      0,
    );
    bands.push({
      name: layer?.name ?? tier.syntheticName ?? UNLAYERED_LABEL,
      index: layer?.index ?? -1,
      y: y0,
      height: tierHeight,
      violationRate: layer ? violationRateFor(layer, data) : 0,
      fileCount: layer?.file_count ?? filesInTier,
      afferent: layer?.afferent ?? 0,
      efferent: layer?.efferent ?? 0,
      instability: layer?.instability ?? null,
    });

    // Left-align nodes within each row so the first N nodes of every
    // tier are immediately visible.
    const xOffset = SIDE_PADDING;
    tier.nodes.forEach((n, i) => {
      const row = Math.floor(i / MAX_PER_ROW);
      const col = i % MAX_PER_ROW;
      const nodeW = nodeWidthFor(n);
      const nodeH = NODE_LEAF_H;
      nodes.push({
        id: n.id,
        label: displayLabel(n),
        sublabel: n.id,
        kind: n.kind,
        layer: n.layer,
        layerIndex: layer?.index ?? -1,
        x: xOffset + col * (NODE_LEAF_W + NODE_PADDING) + (NODE_LEAF_W - nodeW) / 2,
        y: y0 + TIER_PADDING_TOP + row * (NODE_LEAF_H + ROW_GAP),
        width: nodeW,
        height: nodeH,
        fileCount: typeof n.metrics.file_count === "number" ? n.metrics.file_count : 1,
      });
    });
    y0 += tierHeight;
  });

  // 5. Route edges between nodes (skip edges that touch a missing node).
  const byId = new Map(nodes.map((n) => [n.id, n] as const));
  const tierOf = new Map<string, number>();
  const rowOf = new Map<string, number>();
  tiers.forEach((tier, i) =>
    tier.nodes.forEach((n, j) => {
      tierOf.set(n.id, i);
      rowOf.set(n.id, Math.floor(j / MAX_PER_ROW));
    }),
  );
  const lastRowOf = (tierIdx: number) => rowsOf(tiers[tierIdx]) - 1;
  const cycleEdgeSet = buildCycleEdgeSet(data);
  const drawable = data.edges.filter((e) => byId.has(e.from) && byId.has(e.to));
  const slots = assignSlots(drawable, byId);
  // Right edge of the widest node per tier: a lane hugs the tiers its
  // edge actually spans instead of the widest tier on the canvas.
  const tierRight = tiers.map((_, i) =>
    Math.max(
      SIDE_PADDING,
      ...nodes.filter((n) => tierOf.get(n.id) === i).map((n) => n.x + n.width),
    ),
  );
  const spanRight = (lo: number, hi: number) =>
    Math.max(...tierRight.slice(Math.min(lo, hi), Math.max(lo, hi) + 1));

  let lanes = 0;
  let laneRight = 0;
  const arcsPerTier = new Map<number, number>();
  const edges: PositionedEdge[] = [];
  for (const e of drawable) {
    const a = byId.get(e.from)!;
    const b = byId.get(e.to)!;
    const ta = tierOf.get(e.from) ?? 0;
    const tb = tierOf.get(e.to) ?? 0;
    const sx = slots.out.get(edgeKey(e)) ?? a.x + a.width / 2;
    const tx = slots.in.get(edgeKey(e)) ?? b.x + b.width / 2;
    const aBottom = a.y + a.height;
    const bBottom = b.y + b.height;

    let kind: EdgeRoute;
    let points: Point[];
    if (ta === tb) {
      // Same tier: arc below both nodes into the target's bottom edge.
      const n = arcsPerTier.get(ta) ?? 0;
      arcsPerTier.set(ta, n + 1);
      const dipY = Math.max(aBottom, bBottom) + ARC_DIP + n * ARC_STEP;
      kind = "arc";
      points = [
        { x: sx, y: aBottom },
        { x: sx, y: dipY },
        { x: tx, y: dipY },
        { x: tx, y: bBottom },
      ];
    } else if (
      tb === ta + 1 &&
      rowOf.get(e.from) === lastRowOf(ta) &&
      rowOf.get(e.to) === 0
    ) {
      // Adjacent tiers, bottom row to top row: the gap between them
      // holds no node.
      kind = "direct";
      points = [
        { x: sx, y: aBottom },
        { x: tx, y: b.y },
      ];
    } else {
      // Spans tiers, or points upward: out to a lane beside the nodes.
      const laneX = spanRight(ta, tb) + LANE_GUTTER + lanes * LANE_STEP;
      lanes += 1;
      laneRight = Math.max(laneRight, laneX);
      kind = "lane";
      if (tb > ta) {
        points = [
          { x: sx, y: aBottom },
          { x: sx, y: aBottom + EDGE_STUB },
          { x: laneX, y: aBottom + EDGE_STUB },
          { x: laneX, y: b.y - EDGE_STUB },
          { x: tx, y: b.y - EDGE_STUB },
          { x: tx, y: b.y },
        ];
      } else {
        points = [
          { x: sx, y: aBottom },
          { x: sx, y: aBottom + EDGE_STUB },
          { x: laneX, y: aBottom + EDGE_STUB },
          { x: laneX, y: bBottom + EDGE_STUB },
          { x: tx, y: bBottom + EDGE_STUB },
          { x: tx, y: bBottom },
        ];
      }
    }

    const first = points[0];
    const last = points[points.length - 1];
    edges.push({
      from: e.from,
      to: e.to,
      weight: e.weight,
      isViolation: e.violates_rule !== null,
      violationMessage: e.violates_rule,
      isCycle: cycleEdgeSet.has(edgeKey(e)),
      kind,
      points,
      x1: first.x,
      y1: first.y,
      x2: last.x,
      y2: last.y,
    });
  }

  // Lanes live to the right of the nodes; make room for them.
  const finalWidth = Math.max(width, laneRight + SIDE_PADDING);

  return { width: finalWidth, height, nodes, edges, bands };
}

/**
 * Spread the edges leaving and entering each node along its bottom and top
 * edge, so several edges into one node do not collapse onto one point.
 * Outgoing slots are ordered by the target's x, incoming by the source's,
 * which keeps the fan from crossing itself.
 */
function assignSlots(
  edges: EdgeEntry[],
  byId: Map<string, PositionedNode>,
): { out: Map<string, number>; in: Map<string, number> } {
  const out = new Map<string, number>();
  const inn = new Map<string, number>();
  const outgoing = new Map<string, EdgeEntry[]>();
  const incoming = new Map<string, EdgeEntry[]>();
  for (const e of edges) {
    (outgoing.get(e.from) ?? outgoing.set(e.from, []).get(e.from)!).push(e);
    (incoming.get(e.to) ?? incoming.set(e.to, []).get(e.to)!).push(e);
  }
  const centerX = (id: string) => {
    const n = byId.get(id)!;
    return n.x + n.width / 2;
  };
  for (const [id, list] of outgoing) {
    const n = byId.get(id)!;
    list.sort((p, q) => centerX(p.to) - centerX(q.to));
    list.forEach((e, i) => out.set(edgeKey(e), n.x + (n.width * (i + 1)) / (list.length + 1)));
  }
  for (const [id, list] of incoming) {
    const n = byId.get(id)!;
    list.sort((p, q) => centerX(p.from) - centerX(q.from));
    list.forEach((e, i) => inn.set(edgeKey(e), n.x + (n.width * (i + 1)) / (list.length + 1)));
  }
  return { out, in: inn };
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
