import { useMemo, useState } from "react";
import type { DataContract, NodeEntry } from "../types";
import { DrillBreadcrumb } from "./DrillBreadcrumb";

type Tab = "Info" | "Files" | "Levels" | "Issues" | "Rules";

export interface SidePanelProps {
  data: DataContract;
  scope: string;
  onScope?: (scope: string) => void;
  onSelect?: (id: string) => void;
  onSpotFilter?: (
    f: "all" | "in-cycles" | "with-violations" | "gravity-wells",
  ) => void;
  onScoreClick?: () => void;
  foldersOnly?: boolean;
  onFoldersOnly?: (b: boolean) => void;
  /** Current selected issue key (`${kind}-${index}`) — drives the
   *  sticky selected state on the Issues tab card. Null = nothing. */
  activeIssueKey?: string | null;
  /** Called when the user clicks an issue card. The receiver wires
   *  this into the canvas focus mode (#275). */
  onIssueFocus?: (issue: Issue | null, key: string | null) => void;
}

export function SidePanel({
  data,
  scope,
  onScope,
  onSelect,
  onSpotFilter,
  onScoreClick,
  foldersOnly,
  onFoldersOnly,
  activeIssueKey,
  onIssueFocus,
}: SidePanelProps) {
  const [tab, setTab] = useState<Tab>("Info");
  const issuesCount =
    data.violations.length +
    data.cycles.length +
    data.gravity_wells.length +
    data.red_flags.length;
  return (
    <aside
      id="side-panel"
      className="flex min-h-0 flex-col border-r border-border bg-card"
    >
      <Tabs current={tab} onChange={setTab} issuesCount={issuesCount} />
      <div className="flex-1 overflow-y-auto px-4 py-3.5">
        {tab === "Info" && <InfoTab data={data} onScoreClick={onScoreClick} />}
        {tab === "Files" && (
          <FilesTab
            data={data}
            scope={scope}
            onScope={onScope}
            onSelect={onSelect}
            foldersOnly={foldersOnly ?? false}
            onFoldersOnly={onFoldersOnly}
          />
        )}
        {tab === "Levels" && (
          <LevelsTab
            data={data}
            scope={scope}
            onScope={onScope}
            onSelect={onSelect}
          />
        )}
        {tab === "Issues" && (
          <IssuesTab
            data={data}
            onScope={onScope}
            onSelect={onSelect}
            onSpotFilter={onSpotFilter}
            activeIssueKey={activeIssueKey ?? null}
            onIssueFocus={onIssueFocus}
          />
        )}
        {tab === "Rules" && (
          <RulesTab
            data={data}
            onSpotFilter={onSpotFilter}
            onSelect={onSelect}
          />
        )}
      </div>
    </aside>
  );
}

function Tabs({
  current,
  onChange,
  issuesCount,
}: {
  current: Tab;
  onChange: (t: Tab) => void;
  issuesCount: number;
}) {
  const tabs: Tab[] = ["Info", "Files", "Levels", "Issues", "Rules"];
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
          {t === "Issues" && issuesCount > 0 && (
            <span className="ml-1 rounded-full bg-edge-violation/20 px-1.5 py-0.5 text-[9px] font-bold text-edge-violation">
              {issuesCount > 99 ? "99+" : issuesCount}
            </span>
          )}
        </button>
      ))}
    </div>
  );
}

function InfoTab({
  data,
  onScoreClick,
}: {
  data: DataContract;
  onScoreClick?: () => void;
}) {
  // Score block sits at the very top so the headline number is never
  // pushed below the fold by the AutoLayersBanner (#272).
  return (
    <>
      <ScoreBlock data={data} onScoreClick={onScoreClick} />
      <WelcomeCard data={data} onScoreClick={onScoreClick} />
      <SectionHeading>Stats</SectionHeading>
      <Stats data={data} onScoreClick={onScoreClick} />
      {data.history.length >= 2 && (
        <>
          <SectionHeading>History</SectionHeading>
          <HistoryScrubber data={data} />
        </>
      )}
      {data.layers_auto_detected && <AutoLayersBanner data={data} />}
    </>
  );
}

function ScoreBlock({
  data,
  onScoreClick,
}: {
  data: DataContract;
  onScoreClick?: () => void;
}) {
  // Trend delta = current score − previous snapshot score; only shown
  // when history has ≥2 points (so we have something to subtract).
  const h = data.history;
  const delta =
    h.length >= 2 ? data.health_score - h[h.length - 2].health_score : null;

  return (
    <div className="mb-3 rounded-md border border-border bg-canvas p-4">
      <p className="m-0 mb-1 text-[10px] font-semibold uppercase tracking-wider text-muted">
        Health
      </p>
      <div className="flex items-baseline gap-3">
        <button
          onClick={onScoreClick}
          aria-label="Show health score breakdown"
          className="cursor-pointer rounded-sm border-b border-dotted border-accent-domain/60 text-left font-bold leading-none text-accent-domain hover:bg-accent-domain/10"
          style={{ fontSize: "32px" }}
          title="Why this score? Click for the breakdown."
        >
          {formatScore(data.health_score)}
          <span className="ml-1 text-[14px] font-normal text-muted">/100</span>
        </button>
        {delta !== null && delta !== 0 && (
          <span
            className={
              "font-mono text-[13px] " +
              (delta > 0 ? "text-accent-domain" : "text-edge-violation")
            }
            title="Change since the previous snapshot"
          >
            {delta > 0 ? "▲ +" : "▼ "}
            {formatScore(Math.abs(delta))}
          </span>
        )}
      </div>
    </div>
  );
}

function HistoryScrubber({ data }: { data: DataContract }) {
  const points = data.history;
  const [hover, setHover] = useState<number | null>(null);

  // SVG viewbox: 200×40, padded so circles don't clip.
  const W = 200;
  const H = 40;
  const PAD_X = 4;
  const PAD_Y = 4;
  const xs = points.map(
    (_, i) => PAD_X + (i * (W - 2 * PAD_X)) / Math.max(1, points.length - 1),
  );
  // Pin the y-axis to 0..100 so trend is comparable across runs, even
  // if every snapshot scores high (or low).
  const yOf = (s: number) => H - PAD_Y - (s / 100) * (H - 2 * PAD_Y);
  const ys = points.map((p) => yOf(p.health_score));
  const path = points
    .map((_, i) => `${i === 0 ? "M" : "L"} ${xs[i].toFixed(1)} ${ys[i].toFixed(1)}`)
    .join(" ");

  const current = points.length - 1;
  const selected = hover ?? current;
  const sel = points[selected];
  const delta = selected > 0 ? sel.health_score - points[selected - 1].health_score : 0;

  return (
    <div className="rounded-md border border-border bg-canvas p-3">
      <svg
        viewBox={`0 0 ${W} ${H}`}
        className="block h-10 w-full"
        role="img"
        aria-label={`Health score sparkline over ${points.length} snapshots`}
      >
        <path
          d={path}
          fill="none"
          stroke="currentColor"
          strokeWidth="1.5"
          className="text-accent-domain"
        />
        {points.map((p, i) => (
          <circle
            key={p.snapshot_id}
            cx={xs[i]}
            cy={ys[i]}
            r={i === selected ? 3 : 2}
            className={
              i === selected ? "fill-accent-domain" : "fill-accent-domain/50"
            }
            style={{ cursor: "pointer" }}
            onMouseEnter={() => setHover(i)}
            onMouseLeave={() => setHover(null)}
          >
            <title>
              {p.taken_at} — {formatScore(p.health_score)}/100
            </title>
          </circle>
        ))}
      </svg>
      <p className="m-0 mt-2 text-[11px] text-muted">
        <span className="font-mono text-text">{formatTimestamp(sel.taken_at)}</span>{" "}
        ·{" "}
        <strong className="text-accent-domain">
          {formatScore(sel.health_score)}/100
        </strong>
        {delta !== 0 && (
          <span
            className={
              "ml-2 font-mono " +
              (delta > 0 ? "text-accent-domain" : "text-edge-violation")
            }
          >
            {delta > 0 ? "+" : ""}
            {formatScore(delta)}
          </span>
        )}
      </p>
      <p className="m-0 mt-1 text-[10px] text-muted/70">
        {points.length} snapshot{points.length === 1 ? "" : "s"} · scrub the dots
        to see the score at each point.
      </p>
    </div>
  );
}

function formatTimestamp(ts: string): string {
  // SQLite emits "2026-06-05 04:34:13", ISO has a T. Strip seconds for
  // a calmer display in the sparkline footer.
  return ts.replace("T", " ").replace(/:\d{2}(\.\d+)?Z?$/, "");
}

function AutoLayersBanner({ data }: { data: DataContract }) {
  const [copied, setCopied] = useState(false);
  const names = data.layers.map((l) => l.name).join(", ") || "—";

  // Render the inferred layers as a settings.json snippet the user can
  // paste verbatim. Only the fields the user typically authors —
  // omitting derived noise like allow_sibling/reduced_sibling_weight
  // unless the auto-detector set them away from defaults.
  const snippet = JSON.stringify(
    {
      layers: data.layers.map((l) => ({
        name: l.name,
        pattern: l.pattern,
      })),
    },
    null,
    2,
  );

  async function copy() {
    try {
      await navigator.clipboard.writeText(snippet);
      setCopied(true);
      setTimeout(() => setCopied(false), 1800);
    } catch {
      // Older browsers / file:// without permission — fall back to a
      // hidden textarea + execCommand. Still works on macOS Safari.
      const ta = document.createElement("textarea");
      ta.value = snippet;
      ta.style.position = "fixed";
      ta.style.opacity = "0";
      document.body.appendChild(ta);
      ta.select();
      document.execCommand("copy");
      document.body.removeChild(ta);
      setCopied(true);
      setTimeout(() => setCopied(false), 1800);
    }
  }

  return (
    <div
      role="note"
      className="mt-3 rounded-md border border-accent-ui/40 bg-accent-ui/10 px-3 py-2.5 text-[12px] leading-relaxed"
    >
      <div className="mb-0.5 text-[10px] font-semibold uppercase tracking-wider text-accent-ui">
        Layers auto-detected
      </div>
      <p className="m-0 mb-2 text-muted">
        No <code className="font-mono text-text">layers</code> were configured
        in <code className="font-mono text-text">.noupling/settings.json</code>,
        so the Explorer inferred{" "}
        <strong className="text-text">{names}</strong> from path patterns and
        switched the audit to{" "}
        <code className="font-mono text-text">coupling_mode: "actionable"</code>
        {" "}so coarse-pattern sibling coupling doesn't tank the score. Paste
        the snippet below into your settings to take over (and pick your
        own coupling mode while you're there).
      </p>
      <pre className="m-0 overflow-x-auto rounded-sm border border-border bg-canvas p-2 font-mono text-[10px] text-text">
        {snippet}
      </pre>
      <button
        onClick={copy}
        className="mt-2 inline-flex items-center gap-1.5 rounded-sm bg-action px-2.5 py-1 text-[11px] font-semibold text-action-text"
      >
        {copied ? "✓ Copied" : "Copy snippet"}
      </button>
    </div>
  );
}

function WelcomeCard({
  data,
  onScoreClick,
}: {
  data: DataContract;
  onScoreClick?: () => void;
}) {
  const codebaseTitle = data.codebase.path.split("/").filter(Boolean).pop() ?? data.codebase.path;
  return (
    <div className="rounded-md border border-border bg-canvas p-3.5">
      <h4 id="codebase-header" className="m-0 mb-1.5 text-[14px] font-semibold">
        Welcome to {codebaseTitle}
      </h4>
      <p className="m-0 text-[12px] leading-relaxed text-muted">
        {data.codebase.module_count} modules across {data.layers.length} layers.
        Health{" "}
        <ScoreButton score={data.health_score} onClick={onScoreClick} />
        .{" "}
        <span className="text-muted/80">
          Double-click any node to drill in; click for details.
        </span>
      </p>
    </div>
  );
}

function ScoreButton({
  score,
  onClick,
}: {
  score: number;
  onClick?: () => void;
}) {
  if (!onClick) {
    return (
      <strong className="text-accent-domain">{formatScore(score)}/100</strong>
    );
  }
  return (
    <button
      onClick={onClick}
      aria-label="Show health score breakdown"
      className="cursor-pointer rounded-sm border-b border-dotted border-accent-domain/60 px-0.5 font-bold text-accent-domain hover:bg-accent-domain/10"
      title="Why this score? Click for the breakdown."
    >
      {formatScore(score)}/100
    </button>
  );
}

function Stats({
  data,
  onScoreClick,
}: {
  data: DataContract;
  onScoreClick?: () => void;
}) {
  return (
    <div className="flex flex-col gap-1.5">
      <StatRow label="Health">
        <span data-bind="health_score">
          <ScoreButton score={data.health_score} onClick={onScoreClick} />
        </span>
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
  foldersOnly: boolean;
  onFoldersOnly?: (b: boolean) => void;
}

function FilesTab({
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

/**
 * #274 — Finder-style one-level-at-a-time drill-down. Shows the
 * immediate container children of the current shared scope, with a
 * trimmed badge row per node (file count + active violation count).
 * Single click selects (opens DetailsPanel); double-click drills the
 * shared scope. Up/breadcrumb shares <DrillBreadcrumb> with Files.
 */
function LevelsTab({
  data,
  scope,
  onScope,
  onSelect,
}: {
  data: DataContract;
  scope: string;
  onScope?: (scope: string) => void;
  onSelect?: (id: string) => void;
}) {
  const childrenByParent = useMemo(() => {
    const m = new Map<string | null, NodeEntry[]>();
    for (const n of data.nodes) {
      const key = n.parent;
      const arr = m.get(key);
      if (arr) arr.push(n);
      else m.set(key, [n]);
    }
    for (const arr of m.values()) arr.sort((a, b) => a.id.localeCompare(b.id));
    return m;
  }, [data.nodes]);

  // Per-container violation count = number of violations whose
  // offender file sits inside the container. Cheap to compute lazily.
  const violationsByContainer = useMemo(() => {
    const counts = new Map<string, number>();
    for (const v of data.violations) {
      // Walk up the parent chain from the offender's file and credit
      // every ancestor container. Stops when no parent found.
      let cur: string | null = v.edge.from;
      while (cur) {
        counts.set(cur, (counts.get(cur) ?? 0) + 1);
        const node = data.nodes.find((n) => n.id === cur);
        cur = node?.parent ?? null;
      }
    }
    return counts;
  }, [data.violations, data.nodes]);

  const rootKey = scope === "" ? null : scope;
  const children = (childrenByParent.get(rootKey) ?? []).filter(
    (n) => n.kind !== "file",
  );

  return (
    <div>
      <DrillBreadcrumb scope={scope} onScope={(s) => onScope?.(s)} />
      <p className="m-0 mb-2 text-[11px] text-muted">
        One level at a time. Click a row to select; double-click to drill.
      </p>
      {children.length === 0 ? (
        <p className="m-0 text-[12px] text-muted">
          No nested containers at this level.
        </p>
      ) : (
        <ul className="m-0 flex list-none flex-col gap-1 p-0">
          {children.map((n) => {
            const fc =
              typeof n.metrics.file_count === "number"
                ? n.metrics.file_count
                : null;
            const violCount = violationsByContainer.get(n.id) ?? 0;
            return (
              <li key={n.id}>
                <button
                  onClick={() => onSelect?.(n.id)}
                  onDoubleClick={() => onScope?.(n.id)}
                  className="flex w-full items-center justify-between rounded-sm border border-border bg-canvas px-2 py-1.5 text-left text-[12px] transition-colors hover:bg-canvas/60 hover:border-text/30"
                  title={`${n.id} — double-click to drill`}
                >
                  <span className="flex min-w-0 items-center gap-2">
                    <span
                      className={
                        "inline-block h-3 w-0.5 rounded-sm align-middle " +
                        layerAccent(n.layer)
                      }
                    />
                    <span className="truncate font-mono text-text">
                      {basename(n.id)}
                    </span>
                  </span>
                  <span className="ml-2 flex shrink-0 items-center gap-2 text-[10px]">
                    {fc !== null && (
                      <span className="text-muted">{fc}f</span>
                    )}
                    {violCount > 0 && (
                      <span className="rounded-full bg-edge-violation/20 px-1.5 py-0.5 font-bold text-edge-violation">
                        {violCount}
                      </span>
                    )}
                  </span>
                </button>
              </li>
            );
          })}
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

function basename(p: string): string {
  return p.split("/").filter(Boolean).pop() ?? p;
}

interface IssuesTabProps {
  data: DataContract;
  onScope?: (scope: string) => void;
  onSelect?: (id: string) => void;
  onSpotFilter?: (
    f: "all" | "in-cycles" | "with-violations" | "gravity-wells",
  ) => void;
  onIssueFocus?: (issue: Issue | null, key: string | null) => void;
  activeIssueKey?: string | null;
}

function IssuesTab({
  data,
  onScope,
  onSelect,
  onSpotFilter,
  onIssueFocus,
  activeIssueKey,
}: IssuesTabProps) {
  const items = useMemo(() => buildIssues(data), [data]);

  if (items.length === 0) {
    return (
      <p className="m-0 text-[12px] text-muted">
        No issues in scope. The codebase looks clean here.
      </p>
    );
  }

  return (
    <ul className="m-0 flex list-none flex-col gap-1.5 p-0">
      {items.map((it, i) => {
        const key = `${it.kind}-${i}`;
        const selected = activeIssueKey === key;
        return (
          <li key={key}>
            <button
              aria-pressed={selected}
              onClick={() => {
                onIssueFocus?.(it, key);
                // When focus mode is wired, skip the legacy spot-filter
                // call — the focus mode replaces it (and the spot
                // filter would narrow visible nodes to the issue's
                // file ids, hiding the participant containers needed
                // for inline file-level expansion).
                if (onIssueFocus) {
                  if (it.subject) onSelect?.(it.subject);
                  if (it.kind === "red-flag" && it.scope) onScope?.(it.scope);
                  return;
                }
                switch (it.kind) {
                  case "violation":
                    onSpotFilter?.("with-violations");
                    onSelect?.(it.subject);
                    break;
                  case "cycle":
                    onSpotFilter?.("in-cycles");
                    if (it.subject) onSelect?.(it.subject);
                    break;
                  case "red-flag":
                    if (it.scope) onScope?.(it.scope);
                    if (it.subject) onSelect?.(it.subject);
                    break;
                  case "gravity-well":
                    onSpotFilter?.("gravity-wells");
                    onSelect?.(it.subject);
                    break;
                }
              }}
              className={
                "block w-full rounded-md border bg-canvas px-3 py-2 text-left hover:bg-canvas/60 hover:border-text/30 transition-colors " +
                (selected
                  ? "border-l-4 border-l-accent-domain border-border bg-canvas/80"
                  : "border-border")
              }
              data-issue-key={key}
              title={it.description}
            >
            <div className="mb-0.5 flex items-center justify-between">
              <span
                className={
                  "rounded-full px-1.5 py-0.5 text-[9px] font-bold uppercase tracking-wider " +
                  kindClass(it.kind, it.severity)
                }
              >
                {labelFor(it)}
              </span>
              <span className="font-mono text-[10px] text-muted">
                {it.metric ?? ""}
              </span>
            </div>
            <div className="truncate font-mono text-[11px] text-text">
              {it.title}
            </div>
            {it.subtitle && (
              <div className="truncate font-mono text-[10px] text-muted">
                {it.subtitle}
              </div>
            )}
          </button>
        </li>
        );
      })}
    </ul>
  );
}

type IssueKind = "violation" | "cycle" | "red-flag" | "gravity-well";

export interface Issue {
  kind: IssueKind;
  title: string;
  subtitle?: string;
  description: string;
  /** Primary node id to focus / select. */
  subject: string;
  /** Optional scope to drill the canvas to. */
  scope?: string;
  /** "low" / "medium" / "high" — drives the colour chip. */
  severity?: "low" | "medium" | "high";
  /** Right-aligned numeric tag (RRI, cycle size, …). */
  metric?: string;
}

/**
 * Flatten the four issue families into one priority-sorted list.
 *
 * Priority: high-severity violations → cycles → gravity wells → red flags
 * → medium / low violations. Inside each bucket, sort by intensity
 * descending (severity tier for violations, size for cycles, RRI for
 * gravity wells + red flags).
 */
function buildIssues(data: DataContract): Issue[] {
  const sevWeight = (s: "low" | "medium" | "high") =>
    s === "high" ? 3 : s === "medium" ? 2 : 1;
  const violations: Issue[] = data.violations.map((v) => ({
    kind: "violation",
    title: `${basename(v.edge.from)} → ${basename(v.edge.to)}`,
    subtitle: `${v.edge.from} → ${v.edge.to}`,
    description: `Rule violation (severity ${v.severity}): ${v.rule.from} → ${v.rule.to}`,
    subject: v.edge.from,
    severity: v.severity,
    metric: v.severity,
  }));
  violations.sort((a, b) => sevWeight(b.severity!) - sevWeight(a.severity!));

  const cycles: Issue[] = data.cycles.map((c) => ({
    kind: "cycle",
    title: c.members.map(basename).join(" → "),
    // Subtitle surfaces the *break* edge from the minimum cut, not the
    // full traversal path — the full path used to imply that every
    // hop was equally guilty (bug #277). The 2-vs-14 contrast tells
    // you *why* the recommendation favours this side.
    subtitle: cycleSubtitle(c),
    // Tooltip carries the full traversal path so it isn't lost; the
    // DetailsPanel cycle section is the proper home for it.
    description: c.members.join(" → "),
    subject: c.members[0],
    metric: `size ${c.size}`,
  }));
  cycles.sort((a, b) => (b.metric === a.metric ? 0 : b.metric! > a.metric! ? 1 : -1));

  const wells: Issue[] = data.gravity_wells.map((g) => ({
    kind: "gravity-well",
    title: basename(g.module_path),
    subtitle: g.module_path,
    description: `Gravity well: ${g.relationship_count} inbound relationships, total RRI ${g.total_rri.toFixed(1)}`,
    subject: g.module_path,
    metric: `RRI ${g.total_rri.toFixed(1)}`,
  }));
  wells.sort((a, b) => parseFloat((b.metric ?? "0").replace(/[^\d.]/g, "")) - parseFloat((a.metric ?? "0").replace(/[^\d.]/g, "")));

  const flags: Issue[] = data.red_flags.map((f) => ({
    kind: "red-flag",
    title: `${humaniseFlag(f.flag_type)}: ${f.modules.map(basename).join(" + ")}`,
    subtitle: f.modules.join(" / "),
    description: f.recommendation,
    subject: f.modules[0],
    metric: `RRI ${f.rri.toFixed(1)}`,
  }));
  flags.sort((a, b) => parseFloat((b.metric ?? "0").replace(/[^\d.]/g, "")) - parseFloat((a.metric ?? "0").replace(/[^\d.]/g, "")));

  // Interleave: high-sev violations first, then cycles, then gravity wells,
  // then red flags, then medium/low violations.
  const highVios = violations.filter((v) => v.severity === "high");
  const otherVios = violations.filter((v) => v.severity !== "high");
  return [...highVios, ...cycles, ...wells, ...flags, ...otherVios];
}

function labelFor(it: Issue): string {
  switch (it.kind) {
    case "violation":
      return `${it.severity?.toUpperCase()} VIOLATION`;
    case "cycle":
      return "CYCLE";
    case "gravity-well":
      return "GRAVITY WELL";
    case "red-flag":
      return "RED FLAG";
  }
}

function kindClass(kind: IssueKind, severity?: "low" | "medium" | "high"): string {
  if (kind === "violation") {
    if (severity === "high") return "bg-edge-violation/20 text-edge-violation";
    if (severity === "medium") return "bg-accent-infra/20 text-accent-infra";
    return "bg-muted/20 text-muted";
  }
  if (kind === "cycle") return "bg-edge-cycle/20 text-edge-cycle";
  if (kind === "gravity-well") return "bg-accent-infra/20 text-accent-infra";
  return "bg-accent-ui/20 text-accent-ui";
}

/**
 * Cycle-row subtitle. Renders the break edge with weight contrast
 * (`break: A → B (2 vs 14)`) so the recommendation is justified.
 * 2-node cycles where both directions are equally weighted render as
 * `A ⇄ B (equal weight)` — the data doesn't support picking a side.
 */
function cycleSubtitle(c: {
  members: string[];
  minimum_cut: { from: string; to: string; weight: number; vs_weight: number }[];
  size: number;
}): string {
  const cut = c.minimum_cut[0];
  if (!cut) {
    // No min-cut emitted (analyzer fallback path). Show the first hop.
    return `break: ${basename(c.members[0])} → ${basename(c.members[1] ?? "")}`;
  }
  const from = basename(cut.from);
  const to = basename(cut.to);
  // 2-node cycle with equal weights either direction → don't fake a
  // preference. cycle_order is the dir count; size==2 catches mutual.
  if (c.size === 2 && cut.weight === cut.vs_weight) {
    return `${from} ⇄ ${to} (equal weight)`;
  }
  return `break: ${from} → ${to} (${cut.weight} vs ${cut.vs_weight})`;
}

function humaniseFlag(flag: string): string {
  // Rust's Debug format for the enum gives `FusedSibling` / `TrappedChild`.
  return flag.replace(/([a-z])([A-Z])/g, "$1 $2");
}

function RulesTab({
  data,
  onSpotFilter,
  onSelect,
}: {
  data: DataContract;
  onSpotFilter?: (
    f: "all" | "in-cycles" | "with-violations" | "gravity-wells",
  ) => void;
  onSelect?: (id: string) => void;
}) {
  if (data.effective_rules.length === 0) {
    return (
      <p className="m-0 text-[12px] text-muted">
        No layer rules or dependency rules declared. Add a{" "}
        <code className="font-mono text-text">layers</code> array or{" "}
        <code className="font-mono text-text">dependency_rules</code> to{" "}
        <code className="font-mono text-text">.noupling/settings.json</code> to
        start enforcing architecture.
      </p>
    );
  }
  // Sort rules with violations first (descending count), then the rest
  // alphabetically — surfaces what's actively breaking the codebase.
  const sorted = [...data.effective_rules].sort((a, b) => {
    if (a.current_violation_count !== b.current_violation_count) {
      return b.current_violation_count - a.current_violation_count;
    }
    return (a.from + a.to).localeCompare(b.from + b.to);
  });
  return (
    <ul className="m-0 flex list-none flex-col gap-2 p-0">
      {sorted.map((r, i) => {
        const broken = r.current_violation_count > 0;
        return (
          <li key={i}>
            <button
              onClick={() => {
                if (!broken) return;
                onSpotFilter?.("with-violations");
                // Jump to the first concrete offender of this rule so the
                // user lands on something — not just "filter is now
                // active, hunt around."
                const first = data.violations.find(
                  (v) => v.rule.from === r.from && v.rule.to === r.to,
                );
                if (first) onSelect?.(first.edge.from);
              }}
              disabled={!broken}
              className={
                "block w-full rounded-sm border p-2.5 text-left text-[12px] " +
                (broken
                  ? "border-edge-violation/40 bg-edge-violation/5 hover:bg-edge-violation/10"
                  : "border-border bg-canvas cursor-default")
              }
              title={
                broken
                  ? "Filter the canvas to nodes that violate this rule"
                  : "No active violations of this rule"
              }
            >
              <div className="mb-1 flex items-center justify-between gap-2">
                <span className="truncate font-mono text-[10px] text-muted">
                  {r.from} → {r.to}
                </span>
                <div className="flex shrink-0 items-center gap-1.5">
                  {broken && (
                    <span className="rounded-full bg-edge-violation/20 px-1.5 py-0.5 text-[10px] font-bold text-edge-violation">
                      {r.current_violation_count}
                    </span>
                  )}
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
              </div>
              <p className="m-0 text-[11px] leading-relaxed text-muted">
                {r.message}
              </p>
            </button>
          </li>
        );
      })}
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
