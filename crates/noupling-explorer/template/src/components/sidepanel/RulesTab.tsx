import type { DataContract } from "../../types";

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
