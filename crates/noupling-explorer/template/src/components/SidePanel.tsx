import { useState } from "react";
import type { DataContract } from "../types";
import { totalIssueCount } from "../state/queries";
import { FilesTab } from "./sidepanel/FilesTab";
import { InfoTab } from "./sidepanel/InfoTab";
import { IssuesTab, type Issue } from "./sidepanel/IssuesTab";
import { LevelsTab } from "./sidepanel/LevelsTab";
import { RulesTab } from "./sidepanel/RulesTab";

export type { Issue };

type Tab = "Info" | "Files" | "Levels" | "Issues" | "Rules";

export interface SidePanelProps {
  data: DataContract;
  scope: string;
  /** The scope the Explorer opens at; the Info tab treats it as "whole project". */
  homeScope?: string;
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
  homeScope,
}: SidePanelProps) {
  const [tab, setTab] = useState<Tab>("Info");
  const issuesCount = totalIssueCount(data);
  return (
    <aside
      id="side-panel"
      className="flex min-h-0 flex-col border-r border-border bg-card"
    >
      <Tabs current={tab} onChange={setTab} issuesCount={issuesCount} />
      <div className="flex-1 overflow-y-auto px-4 py-3.5">
        {tab === "Info" && (
          <InfoTab
            data={data}
            scope={scope === (homeScope ?? "") ? "" : scope}
            onScoreClick={onScoreClick}
          />
        )}
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
  const tabDescriptions: Record<Tab, string> = {
    Info: "Headline numbers — health score, modules/files counts, history sparkline, auto-layer banner.",
    Files: "Full folder tree of the codebase. Expand and collapse rows; the row affordance drills into a folder.",
    Levels: "Finder-style one-level-at-a-time browser of containers. Single-click selects, double-click drills.",
    Issues: "Every violation, cycle, gravity well, and red flag in priority order. Click to focus the canvas.",
    Rules: "The effective dependency rules (layer order + explicit rules). Broken rules sort first.",
  };
  return (
    <div className="flex border-b border-border">
      {tabs.map((t) => (
        <button
          key={t}
          onClick={() => onChange(t)}
          title={tabDescriptions[t]}
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
