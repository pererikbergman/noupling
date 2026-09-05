import type { IssueFocus } from "./issueFocus";

/**
 * Resolved edge accent — the single answer to "how should this edge
 * render?" Precedence (high → low):
 *
 *   selected → path → minCut → violation → cycle → default
 *
 * Centralised here so the LSM EdgePath stops re-deriving the rule
 * and so a new accent kind only needs the policy module touched
 * (not every view that draws edges).
 */
export type EdgeAccent =
  | "selected"
  | "path"
  | "minCut"
  | "violation"
  | "cycle"
  | "default";

/**
 * Canvas-wide highlight rules, packaged so CanvasArea + the four
 * views consume one value instead of eight separately-threaded
 * props. The non-method fields are the "what to render" knobs the
 * views still need to set their own visuals (band tints, cycle
 * badges, …); the methods are the resolved precedence decisions.
 */
export interface HighlightPolicy {
  /** Render violation edges in red dashed instead of muted gray. */
  highlightViolations: boolean;
  /** Render cycle edges in red + node cycle badges. */
  highlightCycles: boolean;
  /** Tint layer bands with their identity hue (separate from health). */
  layerOverlay: boolean;
  /** Issue-focus state — drives container expansion on the canvas. */
  issueFocus: IssueFocus | null;
  /** The single edge the user explicitly selected, mirrored here so
   *  views that need the endpoints (e.g. Matrix's cell border) don't
   *  have to scan via `edgeAccent`. */
  selectedEdge: { from: string; to: string } | null;

  /**
   * The single accent that wins for a given edge after precedence
   * is applied. `isViolation`/`isCycle` are the edge's intrinsic
   * properties from the layout; the policy combines them with the
   * user-driven highlight sets to pick one accent.
   */
  edgeAccent(
    from: string,
    to: string,
    isViolation: boolean,
    isCycle: boolean,
  ): EdgeAccent;

  /** Number of cycles `id` participates in, or 0 if cycles are hidden. */
  cycleBadgeCount(id: string): number;

  /**
   * Under issue focus, whether a node is one of the participants (or a
   * container holding one) or just there for orientation (#335). `null`
   * when no Issue is focused, so views leave their normal styling alone.
   */
  nodeEmphasis(id: string): NodeEmphasis | null;

  /** Same for an edge: offending edges keep full weight, the rest dim. */
  edgeEmphasis(from: string, to: string): NodeEmphasis | null;
}

export type NodeEmphasis = "participant" | "dimmed";

export interface HighlightInputs {
  /** Edges currently highlighted as the path-finder result. */
  pathEdges: Set<string>;
  /** Edges currently highlighted as the min-cut suggestion (also
   *  carries issue-focus edges — they participate in the same
   *  precedence slot). */
  minCutEdges: Set<string>;
  /** Per-node cycle membership counts for badges. */
  cyclesByNode: Map<string, number>;
  /** The single edge the user explicitly selected on the canvas. */
  selectedEdge: { from: string; to: string } | null;
  /** Toggle: violation edges visible. */
  highlightViolations: boolean;
  /** Toggle: cycle edges + badges visible. */
  highlightCycles: boolean;
  /** Toggle: layer-identity overlay tint visible. */
  layerOverlay: boolean;
  /** Optional issue-focus value (#316). */
  issueFocus: IssueFocus | null;
}

/**
 * Build a HighlightPolicy. Pure; safe to call inside a useMemo.
 */
export function buildHighlightPolicy(
  inputs: HighlightInputs,
): HighlightPolicy {
  const {
    pathEdges,
    minCutEdges,
    cyclesByNode,
    selectedEdge,
    highlightViolations,
    highlightCycles,
    layerOverlay,
    issueFocus,
  } = inputs;

  const selectedKey = selectedEdge
    ? `${selectedEdge.from}→${selectedEdge.to}`
    : null;
  const focusIds = issueFocus
    ? new Set([...issueFocus.participantFiles, ...issueFocus.expandedContainers])
    : null;
  const isParticipant = (id: string) => {
    if (!focusIds) return false;
    if (focusIds.has(id)) return true;
    for (const f of focusIds) {
      // A card standing in for a collapsed chain or a package that holds a
      // participant counts as one …
      if (f.startsWith(id + "/")) return true;
      // … and so does anything inside a participant directory (the children
      // shown when a directory-shaped Issue is focused).
      if (id.startsWith(f + "/")) return true;
    }
    return false;
  };

  return {
    highlightViolations,
    highlightCycles,
    layerOverlay,
    issueFocus,
    selectedEdge,
    edgeAccent(from, to, isViolation, isCycle) {
      const key = `${from}→${to}`;
      if (selectedKey === key) return "selected";
      if (pathEdges.has(key)) return "path";
      if (minCutEdges.has(key)) return "minCut";
      if (isViolation && highlightViolations) return "violation";
      if (isCycle && highlightCycles) return "cycle";
      return "default";
    },
    cycleBadgeCount(id) {
      return highlightCycles ? cyclesByNode.get(id) ?? 0 : 0;
    },
    nodeEmphasis(id) {
      if (!focusIds) return null;
      return isParticipant(id) ? "participant" : "dimmed";
    },
    edgeEmphasis(from, to) {
      if (!issueFocus) return null;
      if (issueFocus.edges.has(`${from}→${to}`)) return "participant";
      // An aggregated edge between two participant cards is the Issue's edge.
      return isParticipant(from) && isParticipant(to) ? "participant" : "dimmed";
    },
  };
}

