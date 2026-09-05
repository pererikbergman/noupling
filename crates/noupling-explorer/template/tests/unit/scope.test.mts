import { test } from "node:test";
import assert from "node:assert/strict";
import { homeScope, clampScope } from "../../src/state/explorerState.ts";
import { displayLabel, collapsedLabel } from "../../src/state/labels.ts";
import type { DataContract, NodeEntry } from "../../src/types.ts";

const node = (id: string, kind: NodeEntry["kind"], parent: string | null): NodeEntry =>
  ({ id, kind, parent, layer: null, metrics: {} });
const data = (nodes: NodeEntry[]) => ({ nodes } as unknown as DataContract);

// noupling's own shape: root → crates → {noupling-cli, noupling-core, noupling-explorer}
const noupling = data([
  node("crates", "container", null),
  node("crates/noupling-cli", "container", "crates"),
  node("crates/noupling-cli/src", "package", "crates/noupling-cli"),
  node("crates/noupling-core", "container", "crates"),
  node("crates/noupling-core/src", "package", "crates/noupling-core"),
  node("crates/noupling-explorer", "container", "crates"),
]);

test("home scope skips single-child chains so the first view has more than one node (#397)", () => {
  assert.equal(homeScope(noupling), "crates");
});

test("home scope stays at root when the root already branches", () => {
  const flat = data([node("app", "package", null), node("lib", "package", null)]);
  assert.equal(homeScope(flat), "");
});

test("home scope enters a lone directory and stops where files or branches appear", () => {
  const withFiles = data([
    node("src", "package", null),
    node("src/main.rs", "file", "src"),
    node("src/core", "package", "src"),
    node("src/core/a.rs", "file", "src/core"),
  ]);
  assert.equal(homeScope(withFiles), "src");
});

test("scopes above home clamp to home, deeper scopes pass through", () => {
  assert.equal(clampScope("", "crates"), "crates");
  assert.equal(clampScope("crates", "crates"), "crates");
  assert.equal(clampScope("crates/noupling-core", "crates"), "crates/noupling-core");
  assert.equal(clampScope("", ""), "");
});

test("a collapsed node is labelled with the path it stands for, not the bare leaf", () => {
  const start = node("crates/noupling-cli", "container", "crates");
  const end = node("crates/noupling-cli/src", "package", "crates/noupling-cli");
  const labelled = collapsedLabel(start, end);
  assert.equal(displayLabel(labelled), "noupling-cli / src");
  assert.equal(displayLabel(end), "src");
});
