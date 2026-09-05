import { test } from "node:test";
import assert from "node:assert/strict";
import { computeLSMLayout, type PositionedEdge, type PositionedNode } from "../../src/lsm/layout.ts";
import type { DataContract, LayerEntry, NodeEntry } from "../../src/shared/types.ts";

// ── fixtures ──────────────────────────────────────────────────────────

function layer(name: string, index: number): LayerEntry {
  return { name, pattern: `**/${name}/**`, allow_sibling: false, index, file_count: 1, afferent: 0, efferent: 0, instability: null };
}
function pkg(id: string, layerName: string | null): NodeEntry {
  return { id, kind: "package", parent: "src", layer: layerName, metrics: { file_count: 1 } };
}
function contract(nodes: NodeEntry[], edges: Array<[string, string]>, layers: LayerEntry[]): DataContract {
  return {
    nodes, layers,
    edges: edges.map(([from, to]) => ({ from, to, weight: 1, violates_rule: null })),
    cycles: [], violations: [],
  } as unknown as DataContract;
}

/** Does the polyline segment p→q pass through the rectangle of `n` (with a margin)? */
function segmentHitsNode(p: { x: number; y: number }, q: { x: number; y: number }, n: PositionedNode): boolean {
  const m = 2;
  const left = n.x - m, right = n.x + n.width + m, top = n.y - m, bottom = n.y + n.height + m;
  // Liang–Barsky style clip test.
  let t0 = 0, t1 = 1;
  const dx = q.x - p.x, dy = q.y - p.y;
  const clip = (pk: number, qk: number) => {
    if (pk === 0) return qk >= 0;
    const t = qk / pk;
    if (pk < 0) { if (t > t1) return false; if (t > t0) t0 = t; }
    else { if (t < t0) return false; if (t < t1) t1 = t; }
    return true;
  };
  return clip(-dx, p.x - left) && clip(dx, right - p.x) && clip(-dy, p.y - top) && clip(dy, bottom - p.y) && t0 < t1;
}

function crossesForeignNode(e: PositionedEdge, nodes: PositionedNode[]): PositionedNode | null {
  for (const n of nodes) {
    if (n.id === e.from || n.id === e.to) continue;
    for (let i = 0; i + 1 < e.points.length; i++) {
      if (segmentHitsNode(e.points[i], e.points[i + 1], n)) return n;
    }
  }
  return null;
}

// One directory per layer — the shape noupling's own crates produce (#398).
const column = contract(
  [pkg("src/scanner", "scanner"), pkg("src/storage", "storage"), pkg("src/analyzer", "analyzer"), pkg("src/core", "core")],
  [["src/scanner", "src/core"], ["src/storage", "src/core"], ["src/analyzer", "src/core"], ["src/scanner", "src/storage"]],
  [layer("scanner", 0), layer("storage", 1), layer("analyzer", 2), layer("core", 3)],
);

test("an edge spanning several tiers is routed around the nodes in between", () => {
  const l = computeLSMLayout(column);
  for (const e of l.edges) {
    const hit = crossesForeignNode(e, l.nodes);
    assert.equal(hit, null, `${e.from} → ${e.to} passes through ${hit?.id}`);
  }
});

test("edges into the same node fan out to distinct endpoints", () => {
  const l = computeLSMLayout(column);
  const intoCore = l.edges.filter((e) => e.to === "src/core");
  assert.equal(intoCore.length, 3);
  const xs = new Set(intoCore.map((e) => e.points[e.points.length - 1].x));
  assert.equal(xs.size, 3, `endpoints collide: ${[...xs].join(",")}`);
});

test("lane routes widen the layout instead of overprinting the last column", () => {
  const l = computeLSMLayout(column);
  const laneEdges = l.edges.filter((e) => e.kind === "lane");
  assert.ok(laneEdges.length >= 2, "scanner→core and storage→core span tiers");
  const rightMost = Math.max(...l.nodes.map((n) => n.x + n.width));
  for (const e of laneEdges) {
    const laneX = Math.max(...e.points.map((p) => p.x));
    assert.ok(laneX > rightMost, `lane at ${laneX} must sit right of the nodes (${rightMost})`);
    assert.ok(l.width >= laneX + 8, `layout width ${l.width} must include the lane at ${laneX}`);
  }
});

test("a same-tier edge arcs below both nodes and lands on the target's bottom edge", () => {
  const twoInOne = contract(
    [pkg("src/a", "ui"), pkg("src/b", "ui")],
    [["src/a", "src/b"]],
    [layer("ui", 0)],
  );
  const l = computeLSMLayout(twoInOne);
  const e = l.edges[0];
  assert.equal(e.kind, "arc");
  const a = l.nodes.find((n) => n.id === "src/a")!;
  const b = l.nodes.find((n) => n.id === "src/b")!;
  const last = e.points[e.points.length - 1];
  assert.equal(last.y, b.y + b.height);
  for (const p of e.points.slice(1, -1)) assert.ok(p.y > a.y + a.height, "arc dips below the nodes");
});

test("an upward edge (cycle back-edge) is routed around the tiers it crosses", () => {
  const ring = contract(
    [pkg("src/ui", "ui"), pkg("src/domain", "domain"), pkg("src/data", "data")],
    [["src/ui", "src/domain"], ["src/domain", "src/data"], ["src/data", "src/ui"]],
    [layer("ui", 0), layer("domain", 1), layer("data", 2)],
  );
  const l = computeLSMLayout(ring);
  const back = l.edges.find((e) => e.from === "src/data" && e.to === "src/ui")!;
  assert.equal(back.kind, "lane");
  assert.equal(crossesForeignNode(back, l.nodes), null);
  const ui = l.nodes.find((n) => n.id === "src/ui")!;
  assert.equal(back.points[back.points.length - 1].y, ui.y + ui.height, "arrives at the target's bottom edge");
});

test("a wide tier wraps into rows, and an edge from an upper row is routed around the rows below it", () => {
  const many = Array.from({ length: 8 }, (_, i) => pkg(`src/ui/w${i}`, "ui"));
  const wide = contract(
    [...many, pkg("src/data", "data")],
    [["src/ui/w0", "src/data"]],
    [layer("ui", 0), layer("data", 1)],
  );
  const l = computeLSMLayout(wide);
  const ys = new Set(l.nodes.filter((n) => n.layer === "ui").map((n) => n.y));
  assert.equal(ys.size, 2, "eight cards wrap onto two rows");
  assert.ok(l.width < 8 * 200, "the tier does not widen past one row");
  const e = l.edges[0];
  const hit = crossesForeignNode(e, l.nodes);
  assert.equal(hit, null, `${e.from} → ${e.to} passes through ${hit?.id}`);
});
