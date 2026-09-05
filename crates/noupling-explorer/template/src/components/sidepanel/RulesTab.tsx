import type { DataContract } from "../../types";
import { firstViolationForRule, ruleOffenders } from "../../state/queries";
import { bandClass, subjectFull } from "./IssuesTab";

export function RulesTab({
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
  // The offender list: Rule and Layer Violation Issues, as core wrote them.
  const offenders = ruleOffenders(data);
  return (
    <div>
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
                const first = firstViolationForRule(data, r.from, r.to);
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
    {offenders.length > 0 && (
      <section className="mt-3" data-testid="rule-offenders">
        <h3 className="m-0 mb-1.5 text-[10px] font-semibold uppercase tracking-wider text-muted">
          Offenders · {offenders.length}
        </h3>
        <ul className="m-0 flex list-none flex-col gap-1 p-0">
          {offenders.map((i) => (
            <li key={i.fingerprint}>
              <button
                onClick={() => onSelect?.(i.participants[0])}
                className={
                  "block w-full rounded-sm border border-border bg-canvas px-2 py-1.5 text-left hover:bg-canvas/60 " +
                  (i.baselined ? "opacity-60" : "")
                }
                title={`${i.reason} ${i.recommendation}`}
              >
                <div className="mb-0.5 flex items-center gap-1.5">
                  <span
                    className={
                      "rounded-full px-1.5 py-0.5 text-[9px] font-bold uppercase tracking-wider " +
                      bandClass(i.severity)
                    }
                  >
                    {i.severity}
                  </span>
                  <span className="text-[10px] uppercase tracking-wider text-muted">{i.kind_name}</span>
                  {i.baselined && (
                    <span className="rounded-full border border-border px-1.5 py-0.5 text-[9px] uppercase tracking-wider text-muted">
                      accepted
                    </span>
                  )}
                </div>
                <div className="truncate font-mono text-[10px] text-text">{subjectFull(i.subject)}</div>
              </button>
            </li>
          ))}
        </ul>
      </section>
    )}
    </div>
  );
}
