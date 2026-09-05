import type { NodeEntry } from "./types";

/**
 * What a node is called on the canvas and in the tabs.
 *
 * Normally the last path segment. A node that stands in for a collapsed
 * single-child chain (`crates/noupling-cli` → `…/src`, shown as one card)
 * carries the collapsed path in `metrics.display_label`, so two crates'
 * `src` directories never appear as two cards both called `src` (#397).
 */
export function displayLabel(node: NodeEntry): string {
  const custom = node.metrics.display_label;
  if (typeof custom === "string" && custom !== "") return custom;
  return basename(node.id);
}

/**
 * The node shown for a collapsed chain from `start` (the immediate child
 * of the scope) down to `end` (the first node with more than one child),
 * labelled `start / … / end`.
 */
export function collapsedLabel(start: NodeEntry, end: NodeEntry): NodeEntry {
  if (start.id === end.id) return end;
  const rel = end.id.startsWith(start.id + "/") ? end.id.slice(start.id.length + 1) : end.id;
  const label = [basename(start.id), ...rel.split("/").filter(Boolean)].join(" / ");
  return { ...end, metrics: { ...end.metrics, display_label: label } };
}

export function basename(p: string): string {
  return p.split("/").filter(Boolean).pop() ?? p;
}
