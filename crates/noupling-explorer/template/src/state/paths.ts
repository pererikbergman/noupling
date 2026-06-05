import type { DataContract } from "../types";

/**
 * Breadth-first shortest path from → to over `data.edges` (treated as a
 * directed graph). Returns the chain of node ids (inclusive of both
 * endpoints) or `null` when no path exists. Used by:
 *
 *  - Path finder (↣) — highlight a user-chosen route between two nodes.
 *  - Min-cut (⌀) — fall back to the cycle's `minimum_cut` field, or BFS
 *    when the field is empty.
 */
export function shortestPath(
  data: DataContract,
  from: string,
  to: string,
): string[] | null {
  if (from === to) return [from];
  const adj = new Map<string, string[]>();
  for (const e of data.edges) {
    if (!adj.has(e.from)) adj.set(e.from, []);
    adj.get(e.from)!.push(e.to);
  }
  const queue: string[] = [from];
  const prev = new Map<string, string>();
  const seen = new Set<string>([from]);
  while (queue.length > 0) {
    const cur = queue.shift()!;
    for (const next of adj.get(cur) ?? []) {
      if (seen.has(next)) continue;
      seen.add(next);
      prev.set(next, cur);
      if (next === to) {
        const chain: string[] = [to];
        let p: string | undefined = prev.get(to);
        while (p) {
          chain.unshift(p);
          p = prev.get(p);
        }
        return chain;
      }
      queue.push(next);
    }
  }
  return null;
}

/**
 * Turn a node-chain into the set of (from→to) edge keys it traverses.
 * Used to look up edges to highlight in the canvas renderers.
 */
export function pathEdges(chain: string[] | null): Set<string> {
  const out = new Set<string>();
  if (!chain) return out;
  for (let i = 0; i < chain.length - 1; i++) {
    out.add(`${chain[i]}→${chain[i + 1]}`);
  }
  return out;
}

/**
 * Collect the min-cut edges to highlight when the user toggles ⌀ —
 * one (from, to) per cycle in scope. Falls back to a heuristic
 * (cycle's first hop) when `minimum_cut` is empty.
 */
export function minCutEdges(data: DataContract): Set<string> {
  const out = new Set<string>();
  for (const c of data.cycles) {
    if (c.minimum_cut.length > 0) {
      for (const cut of c.minimum_cut) {
        out.add(`${cut.from}→${cut.to}`);
      }
    } else if (c.members.length >= 2) {
      // Default: first cycle hop.
      out.add(`${c.members[0]}→${c.members[1]}`);
    }
  }
  return out;
}
