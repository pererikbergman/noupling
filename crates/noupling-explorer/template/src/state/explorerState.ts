import { useEffect, useState } from "react";
import type { DataContract } from "../types";

/**
 * Explorer-wide UI state. Single owner so siblings (canvas, side panel,
 * breadcrumb) stay coordinated.
 *
 * `scope` is the current drill scope — a path prefix the LSM filters to.
 * Empty string means "whole codebase." Double-click a file → scope to its
 * parent dir; click a breadcrumb segment → scope to that segment.
 *
 * Persistence is keyed by `codebase.path + generated_at` so two different
 * Explorer files don't trample each other's state. Full persistence
 * (focus, search, filters) lands in #241; this hook is the seed.
 */
export interface ExplorerState {
  scope: string;
  setScope: (s: string) => void;
  selected: string | null;
  setSelected: (s: string | null) => void;
}

export function useExplorerState(data: DataContract): ExplorerState {
  const key = storageKey(data);
  const [scope, setScopeRaw] = useState<string>(() => readScope(key));
  const [selected, setSelected] = useState<string | null>(null);

  useEffect(() => {
    try {
      localStorage.setItem(`${key}::scope`, scope);
    } catch {
      // private browsing / quota — swallow per PRD §8.11
    }
  }, [key, scope]);

  return { scope, setScope: setScopeRaw, selected, setSelected };
}

function storageKey(data: DataContract): string {
  return `noupling-explorer::${data.codebase.path}::${data.generated_at}`;
}

function readScope(key: string): string {
  try {
    return localStorage.getItem(`${key}::scope`) ?? "";
  } catch {
    return "";
  }
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
