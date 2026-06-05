import { useMemo, useState } from "react";
import type { DataContract, NodeEntry } from "../types";

type Tab = "Info" | "Files" | "Rules";

export interface SidePanelProps {
  data: DataContract;
  scope: string;
  onScope?: (scope: string) => void;
  onSelect?: (id: string) => void;
}

export function SidePanel({ data, scope, onScope, onSelect }: SidePanelProps) {
  const [tab, setTab] = useState<Tab>("Info");
  return (
    <aside
      id="side-panel"
      className="flex min-h-0 flex-col border-r border-border bg-card"
    >
      <Tabs current={tab} onChange={setTab} />
      <div className="flex-1 overflow-y-auto px-4 py-3.5">
        {tab === "Info" && <InfoTab data={data} />}
        {tab === "Files" && (
          <FilesTab
            data={data}
            scope={scope}
            onScope={onScope}
            onSelect={onSelect}
          />
        )}
        {tab === "Rules" && <RulesTab data={data} />}
      </div>
    </aside>
  );
}

function Tabs({
  current,
  onChange,
}: {
  current: Tab;
  onChange: (t: Tab) => void;
}) {
  const tabs: Tab[] = ["Info", "Files", "Rules"];
  return (
    <div className="flex border-b border-border">
      {tabs.map((t) => (
        <button
          key={t}
          onClick={() => onChange(t)}
          className={
            "flex-1 px-2 py-3 text-[11px] font-semibold uppercase tracking-wider " +
            (current === t
              ? "text-text border-b-2 border-text"
              : "text-muted hover:text-text")
          }
        >
          {t}
        </button>
      ))}
    </div>
  );
}

function InfoTab({ data }: { data: DataContract }) {
  return (
    <>
      <WelcomeCard data={data} />
      {data.layers_auto_detected && <AutoLayersBanner data={data} />}
      <SectionHeading>Stats</SectionHeading>
      <Stats data={data} />
    </>
  );
}

function AutoLayersBanner({ data }: { data: DataContract }) {
  const names = data.layers.map((l) => l.name).join(", ") || "—";
  return (
    <div
      role="note"
      className="mt-3 rounded-md border border-accent-ui/40 bg-accent-ui/10 px-3 py-2.5 text-[12px] leading-relaxed"
    >
      <div className="mb-0.5 text-[10px] font-semibold uppercase tracking-wider text-accent-ui">
        Layers auto-detected
      </div>
      <p className="m-0 text-muted">
        No <code className="font-mono text-text">layers</code> were configured
        in <code className="font-mono text-text">.noupling/settings.json</code>,
        so the Explorer inferred{" "}
        <strong className="text-text">{names}</strong> from path patterns.
        The score reflects this guess. Add a real{" "}
        <code className="font-mono text-text">layers</code> array to your
        settings to take over.
      </p>
    </div>
  );
}

function WelcomeCard({ data }: { data: DataContract }) {
  const codebaseTitle = data.codebase.path.split("/").filter(Boolean).pop() ?? data.codebase.path;
  return (
    <div className="rounded-md border border-border bg-canvas p-3.5">
      <h4 id="codebase-header" className="m-0 mb-1.5 text-[14px] font-semibold">
        Welcome to {codebaseTitle}
      </h4>
      <p className="m-0 text-[12px] leading-relaxed text-muted">
        {data.codebase.module_count} modules across {data.layers.length} layers.
        Health{" "}
        <strong className="text-accent-domain">
          {formatScore(data.health_score)}/100
        </strong>
        .{" "}
        <span className="text-muted/80">
          Double-click any node to drill in; click for details.
        </span>
      </p>
    </div>
  );
}

function Stats({ data }: { data: DataContract }) {
  return (
    <div className="flex flex-col gap-1.5">
      <StatRow label="Health">
        <strong className="text-accent-domain">
          <span data-bind="health_score">{formatScore(data.health_score)}</span>/100
        </strong>
      </StatRow>
      <StatRow label="Violations">
        <strong data-bind="summary_counts.violations">{data.summary_counts.violations}</strong>
      </StatRow>
      <StatRow label="Cycles">
        <strong data-bind="summary_counts.cycles">{data.summary_counts.cycles}</strong>
      </StatRow>
      <StatRow label="Gravity wells">
        <strong data-bind="summary_counts.gravity_wells">{data.summary_counts.gravity_wells}</strong>
      </StatRow>
      <StatRow label="Red flags">
        <strong>{data.summary_counts.red_flags}</strong>
      </StatRow>
      <StatRow label="Modules">
        <strong data-bind="codebase.module_count">{data.codebase.module_count}</strong>
      </StatRow>
      <StatRow label="Files">
        <strong data-bind="codebase.file_count">{data.codebase.file_count}</strong>
      </StatRow>
      <StatRow label="Edges">
        <strong data-bind="codebase.edge_count">{data.codebase.edge_count}</strong>
      </StatRow>
    </div>
  );
}

function StatRow({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div className="flex justify-between text-[13px]">
      <span className="text-muted">{label}</span>
      <span>{children}</span>
    </div>
  );
}

function SectionHeading({ children }: { children: React.ReactNode }) {
  return (
    <h3 className="mb-1.5 mt-4 text-[11px] font-semibold uppercase tracking-wider text-muted">
      {children}
    </h3>
  );
}

interface FilesTabProps {
  data: DataContract;
  scope: string;
  onScope?: (scope: string) => void;
  onSelect?: (id: string) => void;
}

function FilesTab({ data, scope, onScope, onSelect }: FilesTabProps) {
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
  const roots = childrenByParent.get(rootKey) ?? [];
  if (roots.length === 0) {
    return (
      <p className="m-0 text-[12px] text-muted">No files in this scope.</p>
    );
  }

  return (
    <ul className="m-0 flex list-none flex-col gap-0.5 p-0">
      {roots.map((n) => (
        <TreeRow
          key={n.id}
          node={n}
          depth={0}
          childrenByParent={childrenByParent}
          onScope={onScope}
          onSelect={onSelect}
        />
      ))}
    </ul>
  );
}

function TreeRow({
  node,
  depth,
  childrenByParent,
  onScope,
  onSelect,
}: {
  node: NodeEntry;
  depth: number;
  childrenByParent: Map<string | null, NodeEntry[]>;
  onScope?: (scope: string) => void;
  onSelect?: (id: string) => void;
}) {
  // Top-level rows default to expanded so users see the codebase shape
  // immediately; deeper rows default to collapsed so the tree doesn't
  // explode.
  const [expanded, setExpanded] = useState(depth === 0);
  const children = childrenByParent.get(node.id) ?? [];
  const isLeaf = node.kind === "file";
  const label = basename(node.id);

  function onActivate() {
    if (isLeaf) {
      onSelect?.(node.id);
    } else {
      onScope?.(node.id);
    }
  }

  return (
    <li>
      <div
        className="flex cursor-pointer items-center justify-between rounded-sm px-1.5 py-1 text-[12px] hover:bg-canvas"
        style={{ paddingLeft: `${depth * 12 + 6}px` }}
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
          title={node.id}
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
            />
          ))}
        </ul>
      )}
    </li>
  );
}

function basename(p: string): string {
  return p.split("/").filter(Boolean).pop() ?? p;
}

function RulesTab({ data }: { data: DataContract }) {
  return (
    <ul className="flex flex-col gap-2">
      {data.effective_rules.map((r, i) => (
        <li
          key={i}
          className="rounded-sm border border-border bg-canvas p-2.5 text-[12px]"
        >
          <div className="mb-1 flex items-center justify-between">
            <span className="font-mono text-[10px] text-muted">
              {r.from} → {r.to}
            </span>
            <span
              className={
                "rounded-full px-2 py-0.5 text-[10px] " +
                (r.source === "layer_order"
                  ? "bg-accent-domain/20 text-accent-domain"
                  : "bg-accent-ui/20 text-accent-ui")
              }
            >
              {r.source.replace("_", " ")}
            </span>
          </div>
          <p className="m-0 text-[11px] leading-relaxed text-muted">{r.message}</p>
        </li>
      ))}
    </ul>
  );
}

function formatScore(n: number): string {
  // Round to 1 decimal; trim trailing zero so 100.0 → 100.
  const r = Math.round(n * 10) / 10;
  return Number.isInteger(r) ? String(r) : r.toFixed(1);
}

function layerAccent(layer: string | null): string {
  if (!layer) return "bg-muted/30";
  if (layer.includes("ui")) return "bg-accent-ui";
  if (layer.includes("infra")) return "bg-accent-infra";
  return "bg-accent-domain";
}
