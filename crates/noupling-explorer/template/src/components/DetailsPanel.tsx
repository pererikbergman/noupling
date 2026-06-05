import { useEffect, useMemo } from "react";
import type { DataContract, NodeEntry } from "../types";
import { buildSourceUrl } from "../sourceLink";
import {
  VIOLATION_EXPLAINER,
  CYCLE_EXPLAINER,
  GRAVITY_WELL_EXPLAINER,
  RED_FLAG_EXPLAINER,
  type VerdictKind,
} from "../verdictExplainers";

export interface DetailsPanelProps {
  data: DataContract;
  selectedId: string | null;
  onClose: () => void;
  onSelect: (id: string | null) => void;
  /** Focus the whole explorer on this node's sub-tree. Closes the panel. */
  onFocus: (scope: string) => void;
}

export function DetailsPanel({
  data,
  selectedId,
  onClose,
  onSelect,
  onFocus,
}: DetailsPanelProps) {
  // Esc closes (PRD §8.5 acceptance).
  useEffect(() => {
    if (!selectedId) return;
    function onKey(e: KeyboardEvent) {
      if (e.key === "Escape") onClose();
    }
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [selectedId, onClose]);

  const node = useMemo(
    () => (selectedId ? data.nodes.find((n) => n.id === selectedId) : null),
    [selectedId, data.nodes],
  );
  const incoming = useMemo(
    () => (selectedId ? data.edges.filter((e) => e.to === selectedId) : []),
    [selectedId, data.edges],
  );
  const outgoing = useMemo(
    () => (selectedId ? data.edges.filter((e) => e.from === selectedId) : []),
    [selectedId, data.edges],
  );
  const cyclesHere = useMemo(
    () => (selectedId ? data.cycles.filter((c) => c.members.includes(selectedId)) : []),
    [selectedId, data.cycles],
  );
  const violationsHere = useMemo(
    () =>
      selectedId
        ? data.violations.filter(
            (v) => v.edge.from === selectedId || v.edge.to === selectedId,
          )
        : [],
    [selectedId, data.violations],
  );
  const gravityWellsHere = useMemo(
    () =>
      selectedId
        ? data.gravity_wells.filter((g) => g.module_path === selectedId)
        : [],
    [selectedId, data.gravity_wells],
  );
  const redFlagsHere = useMemo(
    () =>
      selectedId
        ? data.red_flags.filter((f) => f.modules.includes(selectedId))
        : [],
    [selectedId, data.red_flags],
  );

  if (!selectedId || !node) return null;

  const url = buildSourceUrl(data.report_options.editor, {
    relPath: node.id,
    codebaseRoot: data.codebase.path,
  });

  return (
    <aside
      role="complementary"
      aria-label={`Details for ${node.id}`}
      // Inlay column in the App's grid (col 3, row 3). No absolute
      // positioning, no shadow — it shares space with the canvas
      // instead of covering it.
      className="flex min-h-0 flex-col border-l border-border bg-card"
    >
      <header className="flex items-center justify-between border-b border-border px-4 py-3">
        <span className="rounded-full bg-success/15 px-2 py-0.5 text-[10px] font-semibold uppercase tracking-wider text-success">
          {node.kind}
        </span>
        <button
          onClick={onClose}
          aria-label="Close details"
          className="rounded-sm px-2 py-1 text-[13px] text-muted hover:bg-canvas hover:text-text"
        >
          ✕ <span className="text-[10px] text-muted/60">esc</span>
        </button>
      </header>

      <div className="flex-1 space-y-4 overflow-y-auto px-4 py-3">
        <section>
          <h2 className="m-0 text-[16px] font-semibold leading-snug">{basename(node.id)}</h2>
          <p className="m-0 mt-0.5 font-mono text-[11px] text-muted">{node.id}</p>
          <p className="m-0 mt-1 text-[11px] text-muted">
            Layer:{" "}
            <span className="text-text">{node.layer ?? "(unlayered)"}</span>
          </p>
        </section>

        <Metrics node={node} />

        <AboutThisVerdict
          violations={violationsHere}
          cycles={cyclesHere}
          gravityWells={gravityWellsHere}
          redFlags={redFlagsHere}
        />

        {violationsHere.length > 0 && (
          <Section title={`Violations · ${violationsHere.length}`}>
            <ul className="space-y-1.5">
              {violationsHere.map((v, i) => (
                <li
                  key={i}
                  className="rounded-sm border border-edge-violation/30 bg-edge-violation/10 p-2 text-[12px]"
                >
                  <div className="font-mono text-[11px] text-muted">
                    {v.edge.from} → {v.edge.to}
                  </div>
                  <div className="mt-0.5 text-[11px] text-text">severity: {v.severity}</div>
                </li>
              ))}
            </ul>
          </Section>
        )}

        {cyclesHere.length > 0 && (
          <Section title={`Cycles · ${cyclesHere.length}`}>
            <ul className="space-y-1.5">
              {cyclesHere.map((c) => (
                <li
                  key={c.id}
                  className="rounded-sm border border-border bg-canvas p-2 text-[12px]"
                >
                  <div className="text-[11px] text-muted">
                    {c.id} · {c.size} members
                  </div>
                  <div className="mt-0.5 font-mono text-[10px] text-muted">
                    {c.members.join(" → ")}
                  </div>
                  {c.minimum_cut.length > 0 && (
                    <div className="mt-1 text-[11px] text-text">
                      Min-cut:{" "}
                      <ul className="mt-0.5 ml-3 list-disc space-y-0.5 text-muted">
                        {c.minimum_cut.map((cut, i) => (
                          <li key={i} className="font-mono">
                            {cut.from} → {cut.to}{" "}
                            <span className="text-muted/70">
                              ({cut.weight} vs {cut.vs_weight})
                            </span>
                          </li>
                        ))}
                      </ul>
                    </div>
                  )}
                </li>
              ))}
            </ul>
          </Section>
        )}

        <DependencyList
          title={`Incoming · ${incoming.length}`}
          edges={incoming}
          pickOther={(e) => e.from}
          onSelect={onSelect}
        />
        <DependencyList
          title={`Outgoing · ${outgoing.length}`}
          edges={outgoing}
          pickOther={(e) => e.to}
          onSelect={onSelect}
        />
      </div>

      <footer className="space-y-2 border-t border-border px-4 py-3">
        <button
          onClick={() => onFocus(focusScopeFor(node))}
          className="block w-full rounded-sm border border-border bg-canvas px-3 py-2 text-center text-[13px] font-semibold text-text hover:bg-pill hover:text-pill-text"
        >
          Focus on this node
        </button>
        <a
          href={url}
          className="block rounded-sm bg-action px-3 py-2 text-center text-[13px] font-semibold text-action-text"
        >
          Open in editor →
        </a>
      </footer>
    </aside>
  );
}

function Metrics({ node }: { node: NodeEntry }) {
  const m = node.metrics;
  const entries: Array<[string, string]> = [];
  if (typeof m.afferent === "number") entries.push(["Ca", String(m.afferent)]);
  if (typeof m.efferent === "number") entries.push(["Ce", String(m.efferent)]);
  if (typeof m.instability === "number")
    entries.push(["I (instability)", m.instability.toFixed(2)]);
  if (typeof m.abstractness === "number")
    entries.push(["A (abstractness)", m.abstractness.toFixed(2)]);
  if (typeof m.distance_from_main_sequence === "number")
    entries.push(["D (distance)", m.distance_from_main_sequence.toFixed(2)]);
  if (m.cohesion === null) entries.push(["Cohesion", "—"]);
  else if (typeof m.cohesion === "number") entries.push(["Cohesion", m.cohesion.toFixed(2)]);
  if (typeof m.file_count === "number") entries.push(["Files", String(m.file_count)]);
  if (typeof m.loc === "number") entries.push(["LOC", String(m.loc)]);

  if (entries.length === 0) return null;
  return (
    <Section title="Metrics">
      <dl className="grid grid-cols-[auto_1fr] gap-x-3 gap-y-1 text-[12px]">
        {entries.map(([k, v]) => (
          <div key={k} className="contents">
            <dt className="text-muted">{k}</dt>
            <dd className="text-right font-mono text-text">{v}</dd>
          </div>
        ))}
      </dl>
    </Section>
  );
}

/**
 * Per-kind verdict explainer block — #276. Renders only the kinds
 * present on the selected node, each with hard-coded prose explaining
 * what the kind means and per-instance numbers showing what tripped
 * the threshold (RRI, severity, cycle break cost, etc.).
 */
function AboutThisVerdict({
  violations,
  cycles,
  gravityWells,
  redFlags,
}: {
  violations: DataContract["violations"];
  cycles: DataContract["cycles"];
  gravityWells: DataContract["gravity_wells"];
  redFlags: DataContract["red_flags"];
}) {
  const blocks: Array<{
    kind: VerdictKind;
    badge: string;
    chipClass: string;
    triggers: string[];
  }> = [];

  if (violations.length > 0) {
    blocks.push({
      kind: VIOLATION_EXPLAINER,
      badge: "VIOLATION",
      chipClass: "bg-edge-violation/20 text-edge-violation",
      triggers: violations.map(
        (v) =>
          `${v.severity} severity: ${v.rule.from} → ${v.rule.to}`,
      ),
    });
  }
  if (cycles.length > 0) {
    blocks.push({
      kind: CYCLE_EXPLAINER,
      badge: "CYCLE",
      chipClass: "bg-edge-cycle/20 text-edge-cycle",
      triggers: cycles.map((c) => {
        const cut = c.minimum_cut[0];
        if (!cut) return `cycle of ${c.size} members`;
        return `cycle of ${c.size} members · break ${basename(cut.from)} → ${basename(cut.to)} (${cut.weight} vs ${cut.vs_weight})`;
      }),
    });
  }
  if (gravityWells.length > 0) {
    blocks.push({
      kind: GRAVITY_WELL_EXPLAINER,
      badge: "GRAVITY WELL",
      chipClass: "bg-accent-infra/20 text-accent-infra",
      triggers: gravityWells.map(
        (g) =>
          `total RRI ${g.total_rri.toFixed(1)} across ${g.relationship_count} relationship${g.relationship_count === 1 ? "" : "s"}`,
      ),
    });
  }
  if (redFlags.length > 0) {
    blocks.push({
      kind: RED_FLAG_EXPLAINER,
      badge: "RED FLAG",
      chipClass: "bg-accent-ui/20 text-accent-ui",
      triggers: redFlags.map(
        (f) =>
          `${f.flag_type.replace(/([a-z])([A-Z])/g, "$1 $2")} (RRI ${f.rri.toFixed(1)}): ${f.recommendation}`,
      ),
    });
  }

  if (blocks.length === 0) return null;
  return (
    <Section title="About this verdict">
      <div className="flex flex-col gap-3">
        {blocks.map((b, i) => (
          <div
            key={i}
            className="rounded-sm border border-border bg-canvas p-2.5 text-[11px] leading-relaxed"
          >
            <div className="mb-1 flex items-center gap-2">
              <span
                className={
                  "rounded-full px-1.5 py-0.5 text-[9px] font-bold uppercase tracking-wider " +
                  b.chipClass
                }
              >
                {b.badge}
              </span>
              <span className="text-[11px] font-semibold text-text">
                {b.kind.title}
              </span>
            </div>
            <p className="m-0 text-muted">{b.kind.what}</p>
            {b.triggers.length > 0 && (
              <ul className="m-0 mt-1.5 ml-3 list-disc text-[10px] text-text">
                {b.triggers.map((t, j) => (
                  <li key={j} className="font-mono">
                    {t}
                  </li>
                ))}
              </ul>
            )}
          </div>
        ))}
      </div>
    </Section>
  );
}

function DependencyList({
  title,
  edges,
  pickOther,
  onSelect,
}: {
  title: string;
  edges: Array<{ from: string; to: string; weight: number; violates_rule: string | null }>;
  pickOther: (e: { from: string; to: string }) => string;
  onSelect: (id: string) => void;
}) {
  if (edges.length === 0) return null;
  const sorted = [...edges].sort((a, b) => b.weight - a.weight);
  return (
    <Section title={title}>
      <ul className="space-y-0.5">
        {sorted.map((e, i) => (
          <li key={i}>
            <button
              onClick={() => onSelect(pickOther(e))}
              className="flex w-full items-center justify-between rounded-sm px-1.5 py-1 text-left text-[12px] hover:bg-canvas"
            >
              <span className="truncate font-mono text-[11px] text-muted">{pickOther(e)}</span>
              <span className="ml-2 text-[10px] text-muted">×{e.weight}</span>
            </button>
          </li>
        ))}
      </ul>
    </Section>
  );
}

function Section({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <section>
      <h3 className="mb-1 text-[10px] font-semibold uppercase tracking-wider text-muted">
        {title}
      </h3>
      {children}
    </section>
  );
}

function basename(p: string): string {
  return p.split("/").filter(Boolean).pop() ?? p;
}

/**
 * For a file node, focus on the containing directory; for a package or
 * container, focus on the node itself.
 */
function focusScopeFor(node: NodeEntry): string {
  if (node.kind === "file") {
    const i = node.id.lastIndexOf("/");
    return i === -1 ? "" : node.id.slice(0, i);
  }
  return node.id;
}
