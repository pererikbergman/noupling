import { useMemo } from "react";
import type { DataContract } from "../../types";
import { basename } from "./shared";

export type IssueKind = "violation" | "cycle" | "red-flag" | "gravity-well";

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
