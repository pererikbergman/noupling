import { useEffect, useMemo } from "react";
import type { DataContract, IssueEntry, IssueKindId, NodeEntry } from "../types";
import { buildSourceUrl } from "../sourceLink";
import {
  cyclesInvolving,
  incomingOf,
  issuesForNode,
  nodeById,
  outgoingOf,
  violationsFor,
} from "../state/queries";
import { KIND_DESCRIPTIONS } from "../verdictExplainers";
import { bandClass, subjectFull, subjectShort } from "./sidepanel/IssuesTab";

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
    () => (selectedId ? nodeById(data, selectedId) ?? null : null),
    [selectedId, data],
  );
  const incoming = useMemo(
    () => (selectedId ? incomingOf(data, selectedId) : []),
    [selectedId, data],
  );
  const outgoing = useMemo(
    () => (selectedId ? outgoingOf(data, selectedId) : []),
    [selectedId, data],
  );
  const cyclesHere = useMemo(
    () => (selectedId ? cyclesInvolving(data, selectedId) : []),
    [selectedId, data],
  );
  const violationsHere = useMemo(
    () => (selectedId ? violationsFor(data, selectedId) : []),
    [selectedId, data],
  );
  const issuesHere = useMemo(
    () => (selectedId ? issuesForNode(data, selectedId) : []),
    [selectedId, data],
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

        <AboutThisVerdict issues={issuesHere} />

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
          className="block w-full rounded-sm border border-border bg-canvas px-3 py-2 text-center text-[13px] font-semibold text-text transition-colors hover:bg-canvas/60 hover:border-text/30"
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
  // A container (only subdirectories) has no coupling metrics of its own —
  // the audit computes Ca/Ce/I/cohesion per package — so list only what is
  // defined for it (#405).
  if (node.kind === "container") {
    if (typeof m.file_count === "number") entries.push(["Files below", String(m.file_count)]);
    if (typeof m.loc === "number" && m.loc > 0) entries.push(["LOC", String(m.loc)]);
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
 * "About this verdict" — #276, reshaped by #345. One block per Issue
 * kind present on the selected node: the kind's background prose from
 * `verdictExplainers` plus, per Issue, the band, subject, reason and
 * recommendation exactly as core wrote them. Nothing here re-derives
 * wording, so the Explorer says what `noupling audit` says.
 */
function AboutThisVerdict({ issues }: { issues: IssueEntry[] }) {
  if (issues.length === 0) return null;
  const kinds: IssueKindId[] = [];
  for (const i of issues) if (!kinds.includes(i.kind)) kinds.push(i.kind);
  return (
    <Section title="About this verdict">
      <div className="flex flex-col gap-3">
        {kinds.map((kind) => {
          const desc = KIND_DESCRIPTIONS[kind];
          const ofKind = issues.filter((i) => i.kind === kind);
          return (
            <div
              key={kind}
              className="rounded-sm border border-border bg-canvas p-2.5 text-[11px] leading-relaxed"
              data-verdict-kind={kind}
            >
              <div className="mb-1 flex items-center gap-2">
                <span className="rounded-full bg-muted/20 px-1.5 py-0.5 text-[9px] font-bold uppercase tracking-wider text-muted">
                  {ofKind[0].kind_name}
                </span>
                <span className="text-[11px] font-semibold text-text">{desc.title}</span>
              </div>
              <p className="m-0 text-muted">{desc.what}</p>
              <ul className="m-0 mt-1.5 flex list-none flex-col gap-1.5 p-0">
                {ofKind.map((i) => (
                  <li
                    key={i.fingerprint}
                    className={"rounded-sm border border-border/60 p-1.5 " + (i.baselined ? "opacity-60" : "")}
                    data-issue-key={i.fingerprint}
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
                      <span
                        className="truncate font-mono text-[10px] text-muted"
                        title={subjectFull(i.subject)}
                      >
                        {subjectShort(i.subject)}
                      </span>
                      {i.baselined && (
                        <span className="rounded-full border border-border px-1.5 py-0.5 text-[9px] uppercase tracking-wider text-muted">
                          accepted
                        </span>
                      )}
                    </div>
                    <p className="m-0 text-[10.5px] text-text" data-role="reason">
                      {i.reason}
                    </p>
                    <p className="m-0 mt-0.5 text-[10.5px] text-text" data-role="recommendation">
                      <span className="text-muted">Do: </span>
                      {i.recommendation}
                    </p>
                    {i.score_impact > 0 && (
                      <p className="m-0 mt-0.5 font-mono text-[10px] text-muted">
                        score impact −{i.score_impact.toFixed(1)}
                      </p>
                    )}
                  </li>
                ))}
              </ul>
            </div>
          );
        })}
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
