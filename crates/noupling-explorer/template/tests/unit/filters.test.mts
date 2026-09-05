import { test } from "node:test";
import assert from "node:assert/strict";
import { applySpotFilter } from "../../src/state/explorerState.ts";
import type { DataContract } from "../../src/types.ts";

// A gravity well two levels down: filtering at the top must keep the
// containers that lead to it, or the canvas goes blank (#402).
const data = {
  nodes: [
    { id: "crates", kind: "container", parent: null, layer: null, metrics: {} },
    { id: "crates/core", kind: "container", parent: "crates", layer: null, metrics: {} },
    { id: "crates/core/src", kind: "package", parent: "crates/core", layer: null, metrics: {} },
    { id: "crates/core/src/mod.rs", kind: "file", parent: "crates/core/src", layer: null, metrics: {} },
    { id: "crates/cli", kind: "container", parent: "crates", layer: null, metrics: {} },
  ],
  edges: [],
  cycles: [],
  violations: [],
  issues: [
    { kind: "gravity_well", participants: ["crates/core/src/mod.rs", "crates/cli"] },
  ],
} as unknown as DataContract;

test("a spot filter keeps every ancestor of a matching node", () => {
  const ids = applySpotFilter("gravity-wells", data)!;
  assert.ok(ids.has("crates/core/src/mod.rs"), "the well itself");
  for (const a of ["crates/core/src", "crates/core", "crates"]) {
    assert.ok(ids.has(a), `ancestor ${a} must stay visible so the well can be reached`);
  }
  assert.ok(!ids.has("crates/cli"), "a sibling that only pulls on the well is not shown");
});
