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
  // Explicit dependency rules, violations first, then alphabetical.
  const explicit = [...data.effective_rules]
    .filter((r) => r.source === "dependency_rule")
    .sort((a, b) => {
      if (a.current_violation_count !== b.current_violation_count) {
        return b.current_violation_count - a.current_violation_count;
      }
      return (a.from + a.to).localeCompare(b.from + b.to);
    });
  // Layer-order rules are one rule — "dependencies flow downward" — that
  // core expands to a pair per (higher, lower) layer. Show the stack once
  // and list only the pairs that are currently broken (#404).
  const layerOrder = data.effective_rules.filter((r) => r.source === "layer_order");
  const layers = [...data.layers].sort((a, b) => a.index - b.index);
  const brokenPairs = layerOrder
    .filter((r) => r.current_violation_count > 0)
    .sort((a, b) => b.current_violation_count - a.current_violation_count);
  const layerBroken = (pattern: string) =>
    brokenPairs
      .filter((r) => r.from === pattern)
      .reduce((acc, r) => acc + r.current_violation_count, 0);
  // The offender list: Rule and Layer Violation Issues, as core wrote them.
  const offenders = ruleOffenders(data);
  const jumpTo = (r: { from: string; to: string }) => {
    onSpotFilter?.("with-violations");
    // Jump to the first concrete offender of this rule so the user lands
    // on something — not just "filter is now active, hunt around."
    const first = firstViolationForRule(data, r.from, r.to);
    if (first) onSelect?.(first.edge.from);
  };
  return (
    <div>
    {layers.length > 0 && layerOrder.length > 0 && (
      <section
        className="mb-3 rounded-sm border border-border bg-canvas p-2.5 text-[12px]"
        data-testid="layer-order"
      >
        <div className="mb-1.5 flex items-center justify-between gap-2">
          <span className="text-[10px] font-semibold uppercase tracking-wider text-muted">
            Layer order
          </span>
          <span className="rounded-full bg-accent-domain/20 px-2 py-0.5 text-[10px] text-accent-domain">
            {layerOrder.length} rule{layerOrder.length === 1 ? "" : "s"}
          </span>
        </div>
        <ol className="m-0 flex list-none flex-col gap-1 p-0">
          {layers.map((l, i) => {
            const broken = layerBroken(l.pattern);
            return (
              <li key={l.name} className="flex items-center gap-2 font-mono text-[11px]">
                <span className="w-4 text-right text-muted">{i + 1}</span>
                <span className="text-text">{l.name}</span>
                <span className="truncate text-muted" title={l.pattern}>
                  {l.pattern}
                </span>
                {broken > 0 && (
                  <span
                    className="ml-auto rounded-full bg-edge-violation/20 px-1.5 py-0.5 text-[10px] font-bold text-edge-violation"
                    title="Imports from this layer that point upward"
                  >
                    {broken}
                  </span>
                )}
              </li>
            );
          })}
        </ol>
        <p className="m-0 mt-1.5 text-[11px] leading-relaxed text-muted">
          A layer may depend on the layers below it, never above.
          {brokenPairs.length === 0 ? " Nothing points upward today." : ""}
        </p>
        {brokenPairs.length > 0 && (
          <ul className="m-0 mt-1.5 flex list-none flex-col gap-1 p-0">
            {brokenPairs.map((r) => (
              <li key={`${r.from}${r.to}`}>
                <button
                  onClick={() => jumpTo(r)}
                  className="flex w-full items-center justify-between gap-2 rounded-sm border border-edge-violation/40 bg-edge-violation/5 px-2 py-1 text-left font-mono text-[11px] hover:bg-edge-violation/10"
                  title="Filter the canvas to nodes that violate this rule"
                >
                  <span className="truncate">
                    {layerNameFor(r.from, layers)} → {layerNameFor(r.to, layers)}
                  </span>
                  <span className="shrink-0 rounded-full bg-edge-violation/20 px-1.5 py-0.5 text-[10px] font-bold text-edge-violation">
                    {r.current_violation_count}
                  </span>
                </button>
              </li>
            ))}
          </ul>
        )}
      </section>
    )}
    {explicit.length > 0 && (
      <h3 className="m-0 mb-1.5 text-[10px] font-semibold uppercase tracking-wider text-muted">
        Dependency rules · {explicit.length}
      </h3>
    )}
    <ul className="m-0 flex list-none flex-col gap-2 p-0">
      {explicit.map((r, i) => {
        const broken = r.current_violation_count > 0;
        return (
          <li key={i} data-testid="dependency-rule">
            <button
              onClick={() => {
                if (!broken) return;
                jumpTo(r);
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
              <div className="mb-1 flex items-start justify-between gap-2">
                <div className="min-w-0 font-mono text-[10px] text-muted">
                  <div className="break-all" data-role="rule-from">{r.from}</div>
                  <div className="break-all">
                    <span className="text-text/60">→ </span>
                    <span data-role="rule-to">{r.to}</span>
                  </div>
                </div>
                <div className="flex shrink-0 items-center gap-1.5">
                  {broken && (
                    <span className="rounded-full bg-edge-violation/20 px-1.5 py-0.5 text-[10px] font-bold text-edge-violation">
                      {r.current_violation_count}
                    </span>
                  )}
                  <span className="rounded-full bg-accent-ui/20 px-2 py-0.5 text-[10px] text-accent-ui">
                    {r.allow ? "allow" : "forbid"}
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

function layerNameFor(pattern: string, layers: DataContract["layers"]): string {
  return layers.find((l) => l.pattern === pattern)?.name ?? pattern;
}
