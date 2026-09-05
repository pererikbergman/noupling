import { useState } from "react";
import type { DataContract } from "../../types";
import {
  ScoreButton,
  SectionHeading,
  StatRow,
  formatScore,
} from "./shared";

export function InfoTab({
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
      <StatRow label="Issues">
        <strong data-bind="summary_counts.issues">{data.summary_counts.issues}</strong>
        {data.summary_counts.baselined_issues > 0 && (
          <span className="ml-1 text-[10px] text-muted">
            ({data.summary_counts.new_issues} new · {data.summary_counts.baselined_issues} baselined)
          </span>
        )}
      </StatRow>
      {data.summary_counts.by_kind
        .filter((k) => k.count > 0)
        .map((k) => (
          <StatRow key={k.kind} label={k.kind_name}>
            <strong data-bind={`summary_counts.by_kind.${k.kind}`}>{k.count}</strong>
          </StatRow>
        ))}
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
