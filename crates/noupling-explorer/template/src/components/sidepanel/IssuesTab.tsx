import type { DataContract, IssueEntry, IssueSubject, SeverityBand } from "../../types";
import { allIssues } from "../../state/queries";
import { basename } from "./shared";

/**
 * The Issues tab renders the Data Contract's `issues` array — the same
 * Issue cards every other format shows (ADR 0002, #345) — in canonical
 * order: severity band, then kind, then subject. Each card carries the
 * kind, band, subject, reason and recommendation written once in core;
 * nothing here re-derives wording. Baselined Issues render dimmed with
 * an "accepted" marker and are excluded from the "new" count.
 */
export type Issue = IssueEntry;

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

export function IssuesTab({
  data,
  onSelect,
  onIssueFocus,
  activeIssueKey,
}: IssuesTabProps) {
  const items = allIssues(data);

  if (items.length === 0) {
    return (
      <p className="m-0 text-[12px] text-muted">
        No issues in scope. The codebase looks clean here.
      </p>
    );
  }

  const baselined = items.filter((i) => i.baselined).length;

  return (
    <div>
      <p
        className="m-0 mb-2 text-[11px] text-muted"
        data-testid="issues-summary"
      >
        <strong className="text-text">{items.length}</strong> issue
        {items.length === 1 ? "" : "s"}
        {baselined > 0 && (
          <>
            {" · "}
            <strong className="text-text">{items.length - baselined}</strong> new
            {" · "}
            <strong className="text-text">{baselined}</strong> baselined
          </>
        )}
      </p>
      <ul className="m-0 flex list-none flex-col gap-1.5 p-0">
        {items.map((it) => {
          const key = it.fingerprint;
          const selected = activeIssueKey === key;
          return (
            <li key={key}>
              <button
                aria-pressed={selected}
                onClick={() => {
                  onIssueFocus?.(it, key);
                  const focus = it.participants[0];
                  if (focus) onSelect?.(focus);
                }}
                className={
                  "block w-full rounded-md border bg-canvas px-3 py-2 text-left transition-colors hover:border-text/30 hover:bg-canvas/60 " +
                  (selected
                    ? "border-l-4 border-l-accent-domain border-border bg-canvas/80"
                    : "border-border") +
                  (it.baselined ? " opacity-60" : "")
                }
                data-issue-key={key}
                data-issue-kind={it.kind}
                data-baselined={it.baselined ? "true" : "false"}
                title={`${it.reason} ${it.recommendation}`}
              >
                <div className="mb-0.5 flex items-center justify-between gap-2">
                  <span className="flex items-center gap-1.5">
                    <span
                      className={
                        "rounded-full px-1.5 py-0.5 text-[9px] font-bold uppercase tracking-wider " +
                        bandClass(it.severity)
                      }
                    >
                      {it.severity}
                    </span>
                    <span className="text-[10px] font-semibold uppercase tracking-wider text-muted">
                      {it.kind_name}
                    </span>
                    {it.baselined && (
                      <span
                        className="rounded-full border border-border px-1.5 py-0.5 text-[9px] uppercase tracking-wider text-muted"
                        data-testid="baselined-marker"
                      >
                        accepted
                      </span>
                    )}
                  </span>
                  <span className="font-mono text-[10px] text-muted">
                    {metricFor(it)}
                  </span>
                </div>
                <div className="truncate font-mono text-[11px] text-text">
                  {subjectShort(it.subject)}
                </div>
                <div className="truncate font-mono text-[10px] text-muted">
                  {subjectFull(it.subject)}
                </div>
              </button>
            </li>
          );
        })}
      </ul>
    </div>
  );
}

/** Colour chip per severity band — one band vocabulary everywhere. */
export function bandClass(band: SeverityBand): string {
  switch (band) {
    case "critical":
      return "bg-edge-cycle/20 text-edge-cycle";
    case "high":
      return "bg-edge-violation/20 text-edge-violation";
    case "medium":
      return "bg-accent-infra/20 text-accent-infra";
    case "low":
      return "bg-muted/20 text-muted";
  }
}

/**
 * Right-aligned figure. A Cycle shows its break edge with weight contrast
 * (`break: A → B (2 vs 14)`, #277) so the recommendation is justified;
 * scoring kinds show their score impact; the rest their key number.
 */
function metricFor(it: IssueEntry): string {
  if (it.kind === "cycle") return cycleBreak(it) ?? `−${it.score_impact.toFixed(1)} pts`;
  if (it.score_impact > 0) return `−${it.score_impact.toFixed(1)} pts`;
  const d = it.detail;
  if (typeof d.total_rri === "number") return `RRI ${d.total_rri.toFixed(0)}`;
  if (typeof d.rri === "number" && it.kind === "red_flag") return `RRI ${d.rri.toFixed(0)}`;
  if (typeof d.distance === "number") return `D ${d.distance.toFixed(2)}`;
  if (typeof d.cohesion === "number") return `cohesion ${d.cohesion.toFixed(2)}`;
  if (typeof d.to_instability === "number" && typeof d.from_instability === "number")
    return `I ${d.from_instability.toFixed(2)} → ${d.to_instability.toFixed(2)}`;
  if (typeof d.line_number === "number") return `line ${d.line_number}`;
  return "";
}

/** `break: A → B (cost vs heaviest other hop)` from the Cycle's detail. */
function cycleBreak(it: IssueEntry): string | null {
  const d = it.detail;
  const link = typeof d.weakest_link === "string" ? d.weakest_link : null;
  const cost = typeof d.break_cost === "number" ? d.break_cost : null;
  const counts = Array.isArray(d.hop_import_counts) ? (d.hop_import_counts as number[]) : [];
  if (!link || cost === null) return null;
  const edge = link.split(" (")[0];
  const [from, to] = edge.split(" -> ");
  if (!from || !to) return null;
  const vs = Math.max(...counts.filter((c) => c !== cost), cost);
  if (counts.length === 2 && counts[0] === counts[1]) {
    return `${basename(from)} ⇄ ${basename(to)} (equal weight)`;
  }
  return `break: ${basename(from)} → ${basename(to)} (${cost} vs ${vs})`;
}

export function subjectShort(s: IssueSubject): string {
  switch (s.type) {
    case "module":
      return basename(s.path);
    case "edge":
      return `${basename(s.from)} → ${basename(s.to)}`;
    case "ring":
      return s.members.map(basename).join(" → ");
  }
}

export function subjectFull(s: IssueSubject): string {
  switch (s.type) {
    case "module":
      return s.path;
    case "edge":
      return `${s.from} → ${s.to}`;
    case "ring":
      return s.members.join(" → ");
  }
}
