import type { DataContract, IssueEntry } from "../shared/types";

/**
 * Canvas-level focus driven by an Issue selection. `App.tsx` keeps
 * one of these in state while the user is drilled in on a single
 * issue; the canvas reads `expandedContainers` + `participantFiles`
 * to decide which containers render as folded cards vs expanded
 * file children, and `edges` to push the offending hops above the
 * default highlight z-order.
 */
export interface IssueFocus {
  /** Stable key matching the IssuesTab's button data-issue-key. */
  key: string;
  /** Lowest common ancestor of the participants — what scope we drilled to. */
  lca: string;
  /** Concrete edges between participants — extra-prominent on the canvas. */
  edges: Set<string>;
  /** Immediate-children-of-LCA containers that hold at least one
   *  participant. These render *expanded* on the canvas (file children
   *  shown inline) instead of as a single container card. */
  expandedContainers: Set<string>;
  /** The actual participant file ids that live under those containers
   *  — the only files to surface in the expanded render so the canvas
   *  doesn't hairball on many-file packages. */
  participantFiles: Set<string>;
}

/**
 * Derive the canvas focus state for a single Issue selection.
 *
 * Pure — no React, no setState. Callers own the side effect of
 * scoping the view to `focus.lca` after consuming the value.
 */
export function computeIssueFocus(
  issue: IssueEntry,
  key: string,
  data: DataContract,
): IssueFocus {
  const participants = participantsOf(issue, data);
  const lca = longestCommonAncestor(participants);
  const edges = edgesBetween(participants, data);

  const expandedContainers = new Set<string>();
  const participantFiles = new Set<string>();
  const lcaPrefix = lca === "" ? "" : lca + "/";
  for (const p of participants) {
    if (!p.startsWith(lcaPrefix) && p !== lca) continue;
    if (p === lca) {
      // A directory-shaped Issue about the scope itself (Zone Flag, Low
      // Cohesion): the whole scope is the participant, not an empty
      // `${lca}/` path that nothing matches (#335 review).
      expandedContainers.add(p);
      continue;
    }
    const rest = p.slice(lcaPrefix.length);
    const firstSegment = rest.split("/")[0];
    const container = lca === "" ? firstSegment : `${lca}/${firstSegment}`;
    expandedContainers.add(container);
    const node = data.nodes.find((n) => n.id === p);
    if (node?.kind === "file") participantFiles.add(p);
  }

  return { key, lca, edges, expandedContainers, participantFiles };
}

/**
 * The participant node ids of an Issue come with the Issue itself
 * (`participants` on the Data Contract, computed in core): both ends of
 * an edge, every member of a ring, the well plus the modules pulling on
 * it, the directory of a directory-shaped Issue.
 */
function participantsOf(it: IssueEntry, _data: DataContract): string[] {
  return it.participants.length > 0 ? it.participants : [];
}

/**
 * Longest common ancestor of a set of paths. Splits each path into
 * `/`-segments and keeps the prefix all paths agree on. Returns `""`
 * when there's nothing in common (drill all the way to root).
 *
 * When the input is a single file path, drops the trailing filename so
 * the focus scope lands on the containing directory.
 */
function longestCommonAncestor(paths: string[]): string {
  if (paths.length === 0) return "";
  const splits = paths.map((p) => p.split("/").filter(Boolean));
  const common: string[] = [];
  for (let i = 0; ; i++) {
    const seg = splits[0][i];
    if (seg === undefined) break;
    if (!splits.every((s) => s[i] === seg)) break;
    common.push(seg);
  }
  if (
    common.length > 0 &&
    paths.length === 1 &&
    common.join("/") === paths[0] &&
    paths[0].includes(".")
  ) {
    common.pop();
  }
  return common.join("/");
}

/**
 * All directed edges between any pair of the supplied participants,
 * encoded as `${from}→${to}` strings — the same key shape the LSM uses
 * for its highlight sets.
 */
function edgesBetween(participants: string[], data: DataContract): Set<string> {
  const out = new Set<string>();
  if (participants.length < 2) return out;
  const set = new Set(participants);
  for (const e of data.edges) {
    if (set.has(e.from) && set.has(e.to)) {
      out.add(`${e.from}→${e.to}`);
    }
  }
  return out;
}
