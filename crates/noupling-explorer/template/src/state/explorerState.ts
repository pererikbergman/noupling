import { useMemo, useState } from "react";
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

export type ViewMode = "lsm" | "matrix" | "composition";

export interface EdgeSelection {
  from: string;
  to: string;
}

export type SpotFilter =
  | "all"
  | "in-cycles"
  | "with-violations"
  | "clean"
  | "hide-violations"
  | "gravity-wells";

/**
 * Two-step path-finder gesture state. `idle` until the user clicks ↣;
 * `pick-from` until they click the first node; `pick-to` until they
 * click the second; then resolves to `done` carrying the from + to
 * node ids so the LSM/Matrix can highlight the resulting path.
 */
export type PathFinder =
  | { mode: "idle" }
  | { mode: "pick-from" }
  | { mode: "pick-to"; from: string }
  | { mode: "done"; from: string; to: string };

/** Field values the store carries. */
export interface ExplorerStateValues {
  scope: string;
  selected: string | null;
  selectedEdge: EdgeSelection | null;
  search: string;
  searchMode: SearchMode;
  spotFilter: SpotFilter;
  layerOverlay: boolean;
  cycleHighlight: boolean;
  viewMode: ViewMode;
  pathFinder: PathFinder;
  minCutShown: boolean;
  /** Files tab folder-only filter (#273). */
  foldersOnly: boolean;
}

/** Store-as-deep-module (#302). One setState; one reset; values
 *  surfaced as fields. Adding a new field is a config entry, not
 *  five edits (state slot + setter + persistence hook + setter in
 *  reset + key suffix in reset). */
export interface ExplorerStore extends ExplorerStateValues {
  setState: (patch: Partial<ExplorerStateValues>) => void;
  /** Wipe all persisted state for this Explorer file and return to defaults (PRD F10.2). */
  reset: () => void;
}

// Backwards-compatible alias — old code that imports `ExplorerState`
// keeps compiling against the new shape. Setter properties (`setScope`,
// `setSelected`, …) are deliberately gone: deepening the interface
// is the whole point of #302.
export type ExplorerState = ExplorerStore;

/**
 * Field config: how each value is persisted (or not), what default it
 * takes, and — for enums — which strings are allowed. One entry per
 * field; the hook reads this once and derives the React state slots,
 * the persistence side-effects, and the reset behaviour.
 *
 * Adding a new field = one entry here. No setter, no manual reset
 * line, no key-suffix string to add elsewhere.
 */
type PersistKind =
  | { kind: "transient" }
  | { kind: "string"; suffix: string }
  | { kind: "bool"; suffix: string }
  | { kind: "enum"; suffix: string; allowed: readonly string[] };

interface FieldSpec<K extends keyof ExplorerStateValues> {
  default: ExplorerStateValues[K];
  persist: PersistKind;
}

const FIELDS: { [K in keyof ExplorerStateValues]: FieldSpec<K> } = {
  scope: { default: "", persist: { kind: "string", suffix: "::scope" } },
  selected: { default: null, persist: { kind: "transient" } },
  selectedEdge: { default: null, persist: { kind: "transient" } },
  search: { default: "", persist: { kind: "string", suffix: "::search" } },
  searchMode: {
    default: "substring",
    persist: {
      kind: "enum",
      suffix: "::searchMode",
      allowed: ["substring", "regex"],
    },
  },
  spotFilter: {
    default: "all",
    persist: {
      kind: "enum",
      suffix: "::spotFilter",
      allowed: [
        "all",
        "in-cycles",
        "with-violations",
        "clean",
        "hide-violations",
        "gravity-wells",
      ],
    },
  },
  layerOverlay: {
    default: false,
    persist: { kind: "bool", suffix: "::layerOverlay" },
  },
  cycleHighlight: {
    default: true,
    persist: { kind: "bool", suffix: "::cycleHighlight" },
  },
  viewMode: {
    default: "lsm",
    persist: {
      kind: "enum",
      suffix: "::viewMode",
      allowed: ["lsm", "matrix", "composition"],
    },
  },
  pathFinder: { default: { mode: "idle" }, persist: { kind: "transient" } },
  minCutShown: { default: false, persist: { kind: "transient" } },
  foldersOnly: {
    default: false,
    persist: { kind: "bool", suffix: "::foldersOnly" },
  },
};

function loadInitial<K extends keyof ExplorerStateValues>(
  key: string,
  spec: FieldSpec<K>,
): ExplorerStateValues[K] {
  const fallback = spec.default;
  try {
    const raw = localStorage.getItem(`${key}${persistSuffix(spec.persist) ?? ""}`);
    if (raw === null) return fallback;
    switch (spec.persist.kind) {
      case "string":
        return raw as ExplorerStateValues[K];
      case "bool":
        return (raw === "1" || raw === "true") as ExplorerStateValues[K];
      case "enum":
        return spec.persist.allowed.includes(raw)
          ? (raw as ExplorerStateValues[K])
          : fallback;
      case "transient":
        return fallback;
    }
  } catch {
    return fallback;
  }
}

function persistSuffix(p: PersistKind): string | null {
  return p.kind === "transient" ? null : p.suffix;
}

function persistValue<K extends keyof ExplorerStateValues>(
  key: string,
  spec: FieldSpec<K>,
  value: ExplorerStateValues[K],
): void {
  if (spec.persist.kind === "transient") return;
  try {
    let raw: string;
    if (spec.persist.kind === "bool") raw = value ? "1" : "0";
    else raw = String(value);
    localStorage.setItem(`${key}${spec.persist.suffix}`, raw);
  } catch {
    /* swallow per PRD §8.11 */
  }
}

const FIELD_KEYS = Object.keys(FIELDS) as Array<keyof ExplorerStateValues>;

export function useExplorerStore(data: DataContract): ExplorerStore {
  const key = storageKey(data);
  // One React state slot per field. The initial value reads through
  // the persistence config, so a new field with `persist: { kind: ... }`
  // automatically gets loaded.
  const [values, setValues] = useState<ExplorerStateValues>(() => {
    const v = {} as ExplorerStateValues;
    for (const k of FIELD_KEYS) {
      // TS narrows the spec per key but the loop is dynamic — cast.
      (v as unknown as Record<string, unknown>)[k] = loadInitial(
        key,
        FIELDS[k] as FieldSpec<typeof k>,
      );
    }
    return v;
  });

  function setState(patch: Partial<ExplorerStateValues>) {
    setValues((prev) => {
      const next = { ...prev, ...patch };
      // Side-effect: persist every patched field that has a non-
      // transient strategy. Iterating the patch (not all fields) keeps
      // writes minimal.
      for (const k of Object.keys(patch) as Array<keyof ExplorerStateValues>) {
        const spec = FIELDS[k] as FieldSpec<typeof k>;
        persistValue(
          key,
          spec,
          (next as unknown as Record<string, unknown>)[
            k
          ] as ExplorerStateValues[typeof k],
        );
      }
      return next;
    });
  }

  function reset() {
    const defaults = {} as ExplorerStateValues;
    for (const k of FIELD_KEYS) {
      (defaults as unknown as Record<string, unknown>)[k] = (
        FIELDS[k] as FieldSpec<typeof k>
      ).default;
    }
    setValues(defaults);
    try {
      for (const k of FIELD_KEYS) {
        const spec = FIELDS[k] as FieldSpec<typeof k>;
        const suffix = persistSuffix(spec.persist);
        if (suffix) localStorage.removeItem(`${key}${suffix}`);
      }
    } catch {
      /* swallow */
    }
  }

  return { ...values, setState, reset };
}

/**
 * @deprecated Use `useExplorerStore` (#302). Kept as a thin alias so
 * any old import paths still compile. The shape returned is the new
 * `ExplorerStore`.
 */
export const useExplorerState = useExplorerStore;


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
 * The scope the Explorer opens at: the root, unless the root is a chain
 * of single-child directories (a repo whose only top-level entry is
 * `crates/`), in which case the first level that branches or holds files.
 * A one-node canvas tells the reader nothing (#397).
 */
export function homeScope(data: DataContract): string {
  let scope = "";
  for (;;) {
    const children = data.nodes.filter((n) =>
      scope === "" ? n.parent === null : n.parent === scope,
    );
    if (children.length !== 1 || children[0].kind === "file") return scope;
    scope = children[0].id;
  }
}

/**
 * Scopes above `home` (including `""`) resolve to `home`; anything at or
 * below it is kept. The stored scope stays `""` for "home", so a report
 * regenerated with a different tree still opens at the right place.
 */
export function clampScope(scope: string, home: string): string {
  if (home === "") return scope;
  if (scope === home || scope.startsWith(home + "/")) return scope;
  return home;
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
    // The well modules themselves — the first participant of each Gravity
    // Well Issue (the rest are the modules pulling on it).
    const ids = new Set<string>();
    for (const i of data.issues) {
      if (i.kind === "gravity_well" && i.participants[0]) ids.add(i.participants[0]);
    }
    return ids;
  }
  // "clean" — nodes that participate in no Issue of any kind.
  const dirty = new Set<string>();
  for (const i of data.issues) for (const id of i.participants) dirty.add(id);
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
