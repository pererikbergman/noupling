import type {
  CycleEntry,
  DataContract,
  EdgeEntry,
  IssueEntry,
  IssueKindId,
  NodeEntry,
  ViolationEntry,
} from "../types";

/**
 * The only module that touches raw `data.*` arrays. Tabs and details
 * panels call named queries here instead of re-deriving the same
 * predicates inline; when the Data Contract shape changes (#319 added
 * `clusters`, future v3 will widen `history`), only this module
 * follows.
 *
 * Issue #320.
 */

// ── List queries ────────────────────────────────────────────────────

export function allViolations(data: DataContract): ViolationEntry[] {
  return data.violations;
}

export function allCycles(data: DataContract): CycleEntry[] {
  return data.cycles;
}

/** Every Issue, in canonical order (band desc, kind, subject). */
export function allIssues(data: DataContract): IssueEntry[] {
  return data.issues;
}

/** Issues whose participants include the supplied node id. */
export function issuesForNode(data: DataContract, id: string): IssueEntry[] {
  return data.issues.filter((i) => i.participants.includes(id));
}

/** Issues of one kind, in canonical order. */
export function issuesOfKind(data: DataContract, kind: IssueKindId): IssueEntry[] {
  return data.issues.filter((i) => i.kind === kind);
}

/**
 * Total issue count surfaced in the side panel tab badges — every kind,
 * baselined included (they are still Issues, just accepted).
 */
export function totalIssueCount(data: DataContract): number {
  return data.issues.length;
}

// ── Node lookups ────────────────────────────────────────────────────

export function nodeById(
  data: DataContract,
  id: string,
): NodeEntry | undefined {
  return data.nodes.find((n) => n.id === id);
}

/**
 * Direct children of a parent node id. Files and packages both surface
 * here — sibling tabs decide whether to filter by `kind`.
 */
export function childrenOf(data: DataContract, parent: string): NodeEntry[] {
  return data.nodes.filter((n) => n.parent === parent);
}

/**
 * Parent → children index. Same shape both FilesTab and LevelsTab
 * build inline today; pulling it here means the tab code stops
 * caring about node iteration order. Each caller still imposes its
 * own sort policy on the returned arrays (file-vs-folder for Files,
 * alphabetical for Levels) — sorting is render-level, not data-level.
 */
export function buildChildIndex(
  data: DataContract,
): Map<string | null, NodeEntry[]> {
  const m = new Map<string | null, NodeEntry[]>();
  for (const n of data.nodes) {
    const key = n.parent;
    const arr = m.get(key);
    if (arr) arr.push(n);
    else m.set(key, [n]);
  }
  return m;
}

// ── Edge queries ────────────────────────────────────────────────────

/** Edges whose target is the supplied id. */
export function incomingOf(data: DataContract, id: string): EdgeEntry[] {
  return data.edges.filter((e) => e.to === id);
}

/** Edges whose source is the supplied id. */
export function outgoingOf(data: DataContract, id: string): EdgeEntry[] {
  return data.edges.filter((e) => e.from === id);
}

/** The aggregated edge between two ids, if one exists. */
export function findEdge(
  data: DataContract,
  from: string,
  to: string,
): EdgeEntry | undefined {
  return data.edges.find((e) => e.from === from && e.to === to);
}

/**
 * File-level imports underlying a (possibly container-aggregated)
 * edge. For file-to-file edges this collapses to the edge itself.
 * Used by the EdgeDetailsPanel's "contributors" section.
 */
export function fileContributorsForEdge(
  data: DataContract,
  from: string,
  to: string,
): EdgeEntry[] {
  const fromPrefix = from + "/";
  const toPrefix = to + "/";
  return data.edges.filter(
    (e) =>
      (e.from === from || e.from.startsWith(fromPrefix)) &&
      (e.to === to || e.to.startsWith(toPrefix)),
  );
}

// ── Issue queries ───────────────────────────────────────────────────

/** Cycles that include the supplied id among their members. */
export function cyclesInvolving(
  data: DataContract,
  id: string,
): CycleEntry[] {
  return data.cycles.filter((c) => c.members.includes(id));
}

/**
 * The first cycle whose member sequence contains a directed `from→to`
 * hop. Used by the EdgeDetailsPanel to surface "this edge participates
 * in cycle X."
 */
export function cycleContainingEdge(
  data: DataContract,
  from: string,
  to: string,
): CycleEntry | undefined {
  return data.cycles.find((c) => {
    for (let i = 0; i < c.members.length; i++) {
      const a = c.members[i];
      const b = c.members[(i + 1) % c.members.length];
      if (a === from && b === to) return true;
    }
    return false;
  });
}

/**
 * Coupling violations whose offending edge has the supplied id at
 * either end.
 */
export function violationsFor(
  data: DataContract,
  id: string,
): ViolationEntry[] {
  return data.violations.filter(
    (v) => v.edge.from === id || v.edge.to === id,
  );
}

/** The violation for a specific directed `from→to` edge, if any. */
export function violationForEdge(
  data: DataContract,
  from: string,
  to: string,
): ViolationEntry | undefined {
  return data.violations.find(
    (v) => v.edge.from === from && v.edge.to === to,
  );
}

/**
 * Per-node cycle membership counts — how many cycles each node id
 * appears in. Used by the LSM to render the cycle-count badge on
 * each node card.
 */
export function cycleMembershipCounts(data: DataContract): Map<string, number> {
  const counts = new Map<string, number>();
  for (const c of data.cycles) {
    for (const id of c.members) {
      counts.set(id, (counts.get(id) ?? 0) + 1);
    }
  }
  return counts;
}

/**
 * First Rule or Layer Violation Issue whose edge matches a rule's
 * `from`/`to` globs — used by the Rules tab to jump to a concrete
 * offender. Falls back to the canvas violation geometry when the
 * Issue carries no matching edge.
 */
export function firstViolationForRule(
  data: DataContract,
  ruleFrom: string,
  ruleTo: string,
): ViolationEntry | undefined {
  return data.violations.find(
    (v) => v.rule.from === ruleFrom && v.rule.to === ruleTo,
  );
}

/** Rule and Layer Violation Issues — the Rules tab's offender list. */
export function ruleOffenders(data: DataContract): IssueEntry[] {
  return data.issues.filter(
    (i) => i.kind === "rule_violation" || i.kind === "layer_violation",
  );
}
