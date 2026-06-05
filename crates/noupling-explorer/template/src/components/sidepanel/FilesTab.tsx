import { useMemo, useState } from "react";
import type { DataContract, NodeEntry } from "../../types";
import { DrillBreadcrumb } from "../DrillBreadcrumb";
import { basename, layerAccent } from "./shared";

interface FilesTabProps {
  data: DataContract;
  scope: string;
  onScope?: (scope: string) => void;
  onSelect?: (id: string) => void;
  foldersOnly: boolean;
  onFoldersOnly?: (b: boolean) => void;
}

export function FilesTab({
  data,
  scope,
  onScope,
  onSelect,
  foldersOnly,
  onFoldersOnly,
}: FilesTabProps) {
  const childrenByParent = useMemo(() => {
    const m = new Map<string | null, NodeEntry[]>();
    for (const n of data.nodes) {
      const key = n.parent;
      const arr = m.get(key);
      if (arr) arr.push(n);
      else m.set(key, [n]);
    }
    for (const arr of m.values()) {
      arr.sort((a, b) => {
        const ak = a.kind === "file" ? 1 : 0;
        const bk = b.kind === "file" ? 1 : 0;
        return ak - bk || a.id.localeCompare(b.id);
      });
    }
    return m;
  }, [data.nodes]);

  // Roots = immediate children of the current drill scope. At scope === ""
  // that's nodes with parent === null (top-level dirs + top-level files);
  // when drilled, it's nodes with parent === scope.
  const rootKey = scope === "" ? null : scope;
  const rawRoots = childrenByParent.get(rootKey) ?? [];
  const roots = foldersOnly
    ? rawRoots.filter((n) => n.kind !== "file")
    : rawRoots;

  return (
    <div>
      <DrillBreadcrumb scope={scope} onScope={(s) => onScope?.(s)} />
      {onFoldersOnly && (
        <div className="mb-2 flex items-center justify-between text-[11px]">
          <span className="text-muted">
            Showing {foldersOnly ? "folders only" : "files + folders"}
          </span>
          <button
            onClick={() => onFoldersOnly(!foldersOnly)}
            aria-pressed={foldersOnly}
            className={
              "rounded-sm border border-border px-2 py-0.5 hover:bg-pill " +
              (foldersOnly ? "bg-pill text-pill-text" : "text-muted")
            }
            title="Toggle whether file leaves are shown"
          >
            {foldersOnly ? "Show files" : "Hide files"}
          </button>
        </div>
      )}
      {roots.length === 0 ? (
        <p className="m-0 text-[12px] text-muted">
          {foldersOnly ? "No folders in this scope." : "No files in this scope."}
        </p>
      ) : (
        <ul className="m-0 flex list-none flex-col gap-0.5 p-0">
          {roots.map((n) => (
            <TreeRow
              key={n.id}
              node={n}
              depth={0}
              childrenByParent={childrenByParent}
              onScope={onScope}
              onSelect={onSelect}
              foldersOnly={foldersOnly}
            />
          ))}
        </ul>
      )}
    </div>
  );
}

function TreeRow({
  node,
  depth,
  childrenByParent,
  onScope,
  onSelect,
  foldersOnly,
}: {
  node: NodeEntry;
  depth: number;
  childrenByParent: Map<string | null, NodeEntry[]>;
  onScope?: (scope: string) => void;
  onSelect?: (id: string) => void;
  foldersOnly: boolean;
}) {
  // Top-level rows default to expanded so users see the codebase shape
  // immediately; deeper rows default to collapsed so the tree doesn't
  // explode.
  const [expanded, setExpanded] = useState(depth === 0);
  const rawChildren = childrenByParent.get(node.id) ?? [];
  const children = foldersOnly
    ? rawChildren.filter((n) => n.kind !== "file")
    : rawChildren;
  const isLeaf = node.kind === "file";
  const label = basename(node.id);

  // #273: body click expands inline (was: drills scope). Double-click
  // is the deliberate drill gesture, so accidental drilling stops.
  function onActivate() {
    if (isLeaf) {
      onSelect?.(node.id);
    } else {
      setExpanded((e) => !e);
    }
  }
  function onDeepDrill() {
    if (!isLeaf) onScope?.(node.id);
  }

  return (
    <li>
      <div
        className="flex cursor-pointer items-center justify-between rounded-sm px-1.5 py-1 text-[12px] hover:bg-canvas"
        style={{ paddingLeft: `${depth * 12 + 6}px` }}
        onDoubleClick={onDeepDrill}
      >
        <button
          onClick={() => !isLeaf && setExpanded((e) => !e)}
          className="mr-1 inline-flex h-4 w-4 items-center justify-center text-muted hover:text-text"
          aria-label={isLeaf ? "Leaf" : expanded ? "Collapse" : "Expand"}
        >
          {isLeaf ? "•" : expanded ? "▾" : "▸"}
        </button>
        <button
          onClick={onActivate}
          title={isLeaf ? node.id : `${node.id} — double-click to drill`}
          className="flex flex-1 min-w-0 items-center gap-2 text-left text-text"
        >
          <span
            className={
              "inline-block h-3 w-0.5 rounded-sm align-middle " +
              layerAccent(node.layer)
            }
          />
          <span className="truncate">{label}</span>
        </button>
        {!isLeaf && (
          <button
            onClick={onDeepDrill}
            aria-label={`Drill into ${label}`}
            title="Drill into this folder (changes shared scope)"
            className="ml-1 rounded-sm px-1 text-[10px] text-muted transition-colors hover:bg-canvas/60 hover:text-text"
          >
            ↘
          </button>
        )}
        <span className="ml-2 text-[10px] text-muted">
          {node.kind === "file"
            ? "file"
            : node.kind === "package"
              ? `${typeof node.metrics.file_count === "number" ? node.metrics.file_count : "?"}f`
              : "▸"}
        </span>
      </div>
      {!isLeaf && expanded && children.length > 0 && (
        <ul className="m-0 list-none p-0">
          {children.map((c) => (
            <TreeRow
              key={c.id}
              node={c}
              depth={depth + 1}
              childrenByParent={childrenByParent}
              onScope={onScope}
              onSelect={onSelect}
              foldersOnly={foldersOnly}
            />
          ))}
        </ul>
      )}
    </li>
  );
}
