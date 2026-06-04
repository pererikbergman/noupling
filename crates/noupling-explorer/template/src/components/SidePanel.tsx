import { useState } from "react";
import type { DataContract } from "../types";

type Tab = "Info" | "Files" | "Rules" | "Plan";

export function SidePanel({ data }: { data: DataContract }) {
  const [tab, setTab] = useState<Tab>("Info");
  return (
    <aside
      id="side-panel"
      className="flex min-h-0 flex-col border-r border-border bg-card"
    >
      <Tabs current={tab} onChange={setTab} />
      <div className="flex-1 overflow-y-auto px-4 py-3.5">
        {tab === "Info" && <InfoTab data={data} />}
        {tab === "Files" && <FilesTab data={data} />}
        {tab === "Rules" && <RulesTab data={data} />}
        {tab === "Plan" && <PlanTab />}
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
  const tabs: Tab[] = ["Info", "Files", "Rules", "Plan"];
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
      <SectionHeading>Steps</SectionHeading>
      <Steps />
      <SectionHeading>Stats</SectionHeading>
      <Stats data={data} />
    </>
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
        . Five-minute tour walks through the architecture.
      </p>
      <button className="mt-3 w-full rounded-sm bg-action px-3 py-2 text-[13px] font-semibold text-action-text">
        Start tour
      </button>
    </div>
  );
}

const STEPS = [
  "Project overview & health",
  "Layer boundaries & rules",
  "Finding the gravity well",
  "Cycle: infra ↔ domain",
  "How to read instability (I)",
  "Composition: drill into a package",
  "Pattern: trapped child",
  "Pattern: fused sibling",
  "Min-cut suggestion",
  "Refactoring plan (v2)",
];

function Steps() {
  return (
    <ol className="m-0 mt-0 list-none p-0">
      {STEPS.map((s, i) => (
        <li
          key={s}
          className={
            "flex cursor-pointer items-baseline gap-2.5 rounded-sm px-1.5 py-2 text-[13px] " +
            (i === 0 ? "bg-canvas text-text" : "text-muted hover:bg-canvas hover:text-text")
          }
        >
          <span className="font-mono text-[10px] text-muted">
            {String(i + 1).padStart(2, "0")}
          </span>
          {s}
        </li>
      ))}
    </ol>
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

function FilesTab({ data }: { data: DataContract }) {
  return (
    <ul className="flex flex-col gap-0.5">
      {data.nodes
        .filter((n) => n.kind !== "container")
        .slice(0, 80)
        .map((n) => (
          <li
            key={n.id}
            className="flex items-center justify-between rounded-sm px-2 py-1.5 text-[12px] hover:bg-canvas"
          >
            <span className="truncate">
              <span
                className={
                  "mr-2 inline-block h-3 w-0.5 rounded-sm align-middle " +
                  layerAccent(n.layer)
                }
              />
              {n.id}
            </span>
            <span className="text-[11px] text-muted">{n.kind}</span>
          </li>
        ))}
    </ul>
  );
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

function PlanTab() {
  return (
    <p className="m-0 text-[12px] leading-relaxed text-muted">
      The refactoring action plan lives here once v2 ships the Sandbox
      experience. Queue moves, splits, and merges; export as JSON or Markdown.
    </p>
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
