import { useEffect, useMemo, useState } from "react";
import type { DataContract, NodeEntry } from "../types";

/**
 * Explorer-wide UI state. Single owner so every consumer (canvas, side
 * panel, breadcrumb, details, search row) stays coordinated.
 *
 * Persistence is keyed by `codebase.path + generated_at` so two different
 * Explorer files don't trample each other's state. The seed wired in #234
 * covers scope + selected; #237/#238/#239/#240 extend it with search and
 * filter state; full persistence with focus + panel layout lands in #241.
 */

export type SearchMode = "substring" | "regex";

export type SpotFilter =
  | "all"
  | "in-cycles"
  | "with-violations"
  | "clean"
  | "hide-violations"
  | "gravity-wells";

export interface ExplorerState {
  scope: string;
  setScope: (s: string) => void;
  selected: string | null;
  setSelected: (s: string | null) => void;
  search: string;
  setSearch: (s: string) => void;
  searchMode: SearchMode;
  setSearchMode: (m: SearchMode) => void;
  spotFilter: SpotFilter;
  setSpotFilter: (f: SpotFilter) => void;
  layerOverlay: boolean;
  setLayerOverlay: (b: boolean) => void;
  cycleHighlight: boolean;
  setCycleHighlight: (b: boolean) => void;
}

export function useExplorerState(data: DataContract): ExplorerState {
  const key = storageKey(data);
  const [scope, setScope] = usePersistedString(`${key}::scope`, "");
  const [selected, setSelected] = useState<string | null>(null);
  const [search, setSearch] = usePersistedString(`${key}::search`, "");
  const [searchMode, setSearchMode] = usePersistedEnum<SearchMode>(
    `${key}::searchMode`,
    "substring",
    ["substring", "regex"],
  );
  const [spotFilter, setSpotFilter] = usePersistedEnum<SpotFilter>(
    `${key}::spotFilter`,
    "all",
    ["all", "in-cycles", "with-violations", "clean", "hide-violations", "gravity-wells"],
  );
  const [layerOverlay, setLayerOverlay] = usePersistedBool(`${key}::layerOverlay`, false);
  const [cycleHighlight, setCycleHighlight] = usePersistedBool(
    `${key}::cycleHighlight`,
    true,
  );

  return {
    scope,
    setScope,
    selected,
    setSelected,
    search,
    setSearch,
    searchMode,
    setSearchMode,
    spotFilter,
    setSpotFilter,
    layerOverlay,
    setLayerOverlay,
    cycleHighlight,
    setCycleHighlight,
  };
}

function usePersistedString(key: string, initial: string) {
  const [v, setV] = useState<string>(() => {
    try {
      return localStorage.getItem(key) ?? initial;
    } catch {
      return initial;
    }
  });
  useEffect(() => {
    try {
      localStorage.setItem(key, v);
    } catch {
      /* swallow per PRD §8.11 */
    }
  }, [key, v]);
  return [v, setV] as const;
}

function usePersistedBool(key: string, initial: boolean) {
  const [v, setV] = useState<boolean>(() => {
    try {
      const raw = localStorage.getItem(key);
      if (raw === null) return initial;
      return raw === "1" || raw === "true";
    } catch {
      return initial;
    }
  });
  useEffect(() => {
    try {
      localStorage.setItem(key, v ? "1" : "0");
    } catch {
      /* swallow */
    }
  }, [key, v]);
  return [v, setV] as const;
}

function usePersistedEnum<T extends string>(key: string, initial: T, allowed: T[]) {
  const [v, setV] = useState<T>(() => {
    try {
      const raw = localStorage.getItem(key);
      if (raw && (allowed as readonly string[]).includes(raw)) return raw as T;
    } catch {
      /* swallow */
    }
    return initial;
  });
  useEffect(() => {
    try {
      localStorage.setItem(key, v);
    } catch {
      /* swallow */
    }
  }, [key, v]);
  return [v, setV] as const;
}

function storageKey(data: DataContract): string {
  return `noupling-explorer::${data.codebase.path}::${data.generated_at}`;
}

/**
 * Parent directory of a path. `"a/b/c.rs"` → `"a/b"`. Returns `""` for
 * top-level files.
 */
export function parentDir(path: string): string {
  const i = path.lastIndexOf("/");
  return i === -1 ? "" : path.slice(0, i);
}

/**
 * Split a scope path into clickable breadcrumb segments. Each segment
 * carries the cumulative prefix used to scope back to that level.
 */
export interface BreadcrumbSegment {
  label: string;
  scope: string;
}
export function breadcrumbFor(scope: string): BreadcrumbSegment[] {
  if (scope === "") return [];
  const parts = scope.split("/").filter(Boolean);
  const out: BreadcrumbSegment[] = [];
  let prefix = "";
  for (const p of parts) {
    prefix = prefix === "" ? p : `${prefix}/${p}`;
    out.push({ label: p, scope: prefix });
  }
  return out;
}

/**
 * True when `path` is inside `scope` (or scope is empty).
 */
export function inScope(path: string, scope: string): boolean {
  if (scope === "") return true;
  return path === scope || path.startsWith(`${scope}/`);
}

/**
 * Compile a search term (substring or regex). Returns a predicate over
 * node id + label. Falsy term → match-all. Invalid regex → match-none.
 */
export function compileSearch(term: string, mode: SearchMode): (n: NodeEntry) => boolean {
  const t = term.trim();
  if (t === "") return () => true;
  if (mode === "regex") {
    try {
      const re = new RegExp(t, "i");
      return (n) => re.test(n.id);
    } catch {
      return () => false;
    }
  }
  const lower = t.toLowerCase();
  return (n) => n.id.toLowerCase().includes(lower);
}

/**
 * Apply a spot filter to the visible node set. Pure predicate over the
 * Data Contract — composes with search + scope by intersection.
 */
export function applySpotFilter(filter: SpotFilter, data: DataContract): Set<string> | null {
  if (filter === "all" || filter === "hide-violations") return null;
  if (filter === "in-cycles") {
    return new Set(data.cycles.flatMap((c) => c.members));
  }
  if (filter === "with-violations") {
    const ids = new Set<string>();
    for (const v of data.violations) {
      ids.add(v.edge.from);
      ids.add(v.edge.to);
    }
    return ids;
  }
  if (filter === "gravity-wells") {
    // The Data Contract doesn't directly enumerate gravity-well node ids,
    // but the summary count comes from `audit_result.gravity_wells`. For now,
    // approximate via nodes with the highest afferent count from edges.
    const incoming = new Map<string, number>();
    for (const e of data.edges) {
      incoming.set(e.to, (incoming.get(e.to) ?? 0) + e.weight);
    }
    const top = [...incoming.entries()]
      .sort((a, b) => b[1] - a[1])
      .slice(0, Math.max(1, data.summary_counts.gravity_wells))
      .map(([id]) => id);
    return new Set(top);
  }
  // "clean" — nodes touched by no violation, no cycle, no red flag.
  const dirty = new Set<string>();
  for (const c of data.cycles) for (const id of c.members) dirty.add(id);
  for (const v of data.violations) {
    dirty.add(v.edge.from);
    dirty.add(v.edge.to);
  }
  const ids = new Set<string>();
  for (const n of data.nodes) if (!dirty.has(n.id)) ids.add(n.id);
  return ids;
}

/**
 * True when violations should be visually highlighted on the canvas.
 * `hide-violations` mutes the red highlights so the structure reads
 * cleanly per PRD F5.5.
 */
export function shouldHighlightViolations(filter: SpotFilter): boolean {
  return filter !== "hide-violations";
}

/**
 * Memoised filter predicate — combines search + spot filter into a
 * single `(node) => boolean` callable.
 */
export function useNodeFilter(data: DataContract, state: ExplorerState) {
  return useMemo(() => {
    const matchesSearch = compileSearch(state.search, state.searchMode);
    const spotSet = applySpotFilter(state.spotFilter, data);
    return (n: NodeEntry) => matchesSearch(n) && (spotSet === null || spotSet.has(n.id));
  }, [data, state.search, state.searchMode, state.spotFilter]);
}
