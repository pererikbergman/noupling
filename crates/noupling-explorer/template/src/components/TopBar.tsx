import { useState } from "react";
import type { DataContract } from "../types";
import type { ViewMode } from "../state/explorerState";
import { HelpDialog } from "./HelpDialog";
import { MethodologyDialog } from "./MethodologyDialog";

export interface TopBarProps {
  data: DataContract;
  theme: "dark" | "light";
  onToggleTheme: () => void;
  onResetView: () => void;
  viewMode: ViewMode;
  onViewMode: (v: ViewMode) => void;
  onStartPathFinder: () => void;
  pathFinderActive: boolean;
  onShowMinCut: () => void;
  minCutShown: boolean;
  hasCycles: boolean;
}

export function TopBar({
  data,
  theme,
  onToggleTheme,
  onResetView,
  viewMode,
  onViewMode,
  onStartPathFinder,
  pathFinderActive,
  onShowMinCut,
  minCutShown,
  hasCycles,
}: TopBarProps) {
  const [helpOpen, setHelpOpen] = useState(false);
  const [guideOpen, setGuideOpen] = useState(false);
  const codebaseTitle = basename(data.codebase.path);
  return (
    <div className="col-span-full flex flex-wrap items-center gap-2.5 border-b border-border bg-card px-4 py-2.5">
      <Logo />
      <Badge>{`v${data.noupling_version}`}</Badge>
      <span className="ml-1 text-[13px] text-muted">{codebaseTitle}</span>

      <Divider />

      {/* View modes — LSM + Matrix are wired; Force + Composition stay as
          v3 placeholders (the PRD §10 advanced views). */}
      <div className="flex items-center gap-1 rounded-md border border-border bg-canvas p-[3px]">
        <ViewBtn
          active={viewMode === "lsm"}
          onClick={() => onViewMode("lsm")}
          title="Layered Structure Map — nodes laid out top-to-bottom by layer; cross-layer edges between them. The headline view."
        >
          LSM
        </ViewBtn>
        <ViewBtn
          active={viewMode === "matrix"}
          onClick={() => onViewMode("matrix")}
          title="N×N dependency heatmap — rows are sources, columns are targets. Cells show edge weight; reds mark cycles + violations."
        >
          Matrix
        </ViewBtn>
        <ViewBtn
          active={viewMode === "force"}
          onClick={() => onViewMode("force")}
          title="Force-directed layout — tightly coupled nodes pull together. Cluster boundaries are precomputed (label propagation)."
        >
          Force
        </ViewBtn>
        <ViewBtn
          active={viewMode === "composition"}
          onClick={() => onViewMode("composition")}
          title="Annotated module map — what each module is. Files inside, layer tag, dominant language, optional LLM purpose label."
        >
          Composition
        </ViewBtn>
      </div>

      <div className="flex-1" />

      <IconButton
        title="Find a dependency path between two nodes — click, then click two cards"
        ariaLabel="Find a dependency path between two nodes"
        active={pathFinderActive}
        onClick={onStartPathFinder}
      >
        ↣
      </IconButton>
      <IconButton
        title={
          hasCycles
            ? "Highlight the minimum cut for every visible cycle"
            : "No cycles in the current scope"
        }
        ariaLabel="Toggle minimum cut highlight"
        active={minCutShown}
        onClick={onShowMinCut}
        disabled={!hasCycles}
      >
        ⌀
      </IconButton>
      <IconButton title="Export the Data Contract JSON" onClick={() => downloadDataContract(data)}>
        ↗
      </IconButton>
      <IconButton title="Reset view to defaults" onClick={onResetView}>
        ↺
      </IconButton>
      <IconButton title="Toggle theme" onClick={onToggleTheme}>
        {theme === "dark" ? "☾" : "☼"}
      </IconButton>
      <IconButton
        title="Field guide — how to read this view, what insights to look for, glossary"
        ariaLabel="Open the Explorer field guide"
        onClick={() => setGuideOpen(true)}
      >
        ⓘ
      </IconButton>
      <IconButton title="Keyboard shortcuts (?)" onClick={() => setHelpOpen(true)}>
        ?
      </IconButton>

      <HelpDialog open={helpOpen} onClose={() => setHelpOpen(false)} />
      <MethodologyDialog open={guideOpen} onClose={() => setGuideOpen(false)} />
    </div>
  );
}

function Logo() {
  return (
    <span className="text-base font-bold tracking-tight">
      <span className="mr-1.5 text-accent-domain">◉</span>noupling
    </span>
  );
}

function Badge({ children }: { children: React.ReactNode }) {
  return (
    <span className="inline-flex items-center gap-1.5 rounded-full bg-success px-2.5 py-0.5 text-[11px] font-semibold text-white before:inline-block before:h-1.5 before:w-1.5 before:rounded-full before:bg-white before:content-['']">
      {children}
    </span>
  );
}

function Divider() {
  return <div className="mx-1 h-5 w-px bg-border" />;
}

function ViewBtn({
  active,
  onClick,
  title,
  children,
}: {
  active?: boolean;
  onClick?: () => void;
  title?: string;
  children: React.ReactNode;
}) {
  return (
    <button
      onClick={onClick}
      aria-pressed={active}
      title={title}
      className={
        "whitespace-nowrap rounded-sm px-2.5 py-1 text-[12px] " +
        (active
          ? "bg-card text-text shadow-[0_1px_0_rgb(var(--border))]"
          : "text-muted hover:text-text")
      }
    >
      {children}
    </button>
  );
}

function IconButton({
  children,
  title,
  ariaLabel,
  onClick,
  active,
  disabled,
}: {
  children: React.ReactNode;
  title: string;
  ariaLabel?: string;
  onClick?: () => void;
  active?: boolean;
  disabled?: boolean;
}) {
  return (
    <button
      title={title}
      aria-label={ariaLabel ?? title}
      aria-pressed={active}
      aria-disabled={disabled}
      onClick={disabled ? undefined : onClick}
      className={
        "inline-flex h-8 w-8 items-center justify-center rounded-sm border text-[14px] " +
        (active
          ? "border-pill bg-pill text-pill-text"
          : disabled
            ? "cursor-not-allowed border-dashed border-border text-muted/50"
            : "border-border text-muted hover:text-text")
      }
    >
      {children}
    </button>
  );
}

function downloadDataContract(data: DataContract) {
  const blob = new Blob([JSON.stringify(data, null, 2)], {
    type: "application/json",
  });
  const url = URL.createObjectURL(blob);
  const a = document.createElement("a");
  a.href = url;
  a.download = `${basename(data.codebase.path) || "codebase"}-explorer-data.json`;
  document.body.appendChild(a);
  a.click();
  document.body.removeChild(a);
  URL.revokeObjectURL(url);
}

function basename(p: string): string {
  return p.split("/").filter(Boolean).pop() ?? p;
}
