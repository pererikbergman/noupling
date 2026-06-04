import { useState } from "react";
import type { DataContract } from "../types";
import { HelpDialog } from "./HelpDialog";

export interface TopBarProps {
  data: DataContract;
  theme: "dark" | "light";
  onToggleTheme: () => void;
  onResetView: () => void;
}

export function TopBar({ data, theme, onToggleTheme, onResetView }: TopBarProps) {
  const [helpOpen, setHelpOpen] = useState(false);
  const codebaseTitle = basename(data.codebase.path);
  return (
    <div className="col-span-2 flex flex-wrap items-center gap-2.5 border-b border-border bg-card px-4 py-2.5">
      <Logo />
      <Badge>{`v${data.noupling_version}`}</Badge>
      <span className="ml-1 text-[13px] text-muted">{codebaseTitle}</span>

      <Divider />

      {/* View modes — only LSM is wired in v1; matrix/force/composition land
          in v3 (PRD §10). Render the placeholders visibly disabled so the
          surface communicates honestly (#254). */}
      <SegmentedGroup
        live={["LSM"]}
        deferred={["Matrix", "Force", "Composition"]}
        active="LSM"
        deferredTooltip="View modes Matrix / Force / Composition ship in v3 (PRD §10)."
      />

      {/* Inside / + External scope — v2 placeholder. */}
      <SegmentedGroup
        live={["Inside"]}
        deferred={["+ External"]}
        active="Inside"
        deferredTooltip="External-dependency overlay ships in v2 (PRD §9)."
      />

      {/* Hide-by-kind chips — placeholder until kind taxonomy is real. */}
      <SegmentedGroup
        live={[]}
        deferred={["UI", "Domain", "Infra", "Tests", "Generated"]}
        active=""
        deferredTooltip="Hide-by-kind filtering needs a kind taxonomy from the scanner — tracked under v2/v3 chrome cleanup (#254)."
      />

      <div className="flex-1" />

      {/* Structure101-style utility icons. ↣ and ⌀ are visibly disabled v2
          placeholders; ▼ is redundant with the spot-filter pills on the
          canvas; ↗ Export is wired; ↺ Reset is wired; ? opens Help. */}
      <DisabledIconButton title="Path finder ships in v2 (PRD §9.x). Tracked in #228.">
        ↣
      </DisabledIconButton>
      <DisabledIconButton title="Cycle min-cut suggestion ships in v2 (PRD §9.x). The cycle data + min-cut already render in the details panel.">
        ⌀
      </DisabledIconButton>
      <DisabledIconButton title="Global filter button is redundant with the spot-filter pills on the canvas. Removed in #254 follow-up.">
        ▼
      </DisabledIconButton>
      <IconButton title="Export the Data Contract JSON" onClick={() => downloadDataContract(data)}>
        ↗
      </IconButton>
      <IconButton title="Reset view to defaults" onClick={onResetView}>
        ↺
      </IconButton>
      <IconButton title="Toggle theme" onClick={onToggleTheme}>
        {theme === "dark" ? "☾" : "☼"}
      </IconButton>
      <IconButton title="Keyboard shortcuts (?)" onClick={() => setHelpOpen(true)}>
        ?
      </IconButton>

      <HelpDialog open={helpOpen} onClose={() => setHelpOpen(false)} />
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

function SegmentedGroup({
  live,
  deferred,
  active,
  deferredTooltip,
}: {
  live: string[];
  deferred: string[];
  active: string;
  deferredTooltip: string;
}) {
  if (live.length === 0 && deferred.length === 0) return null;
  return (
    <div className="flex items-center gap-1 rounded-md border border-border bg-canvas p-[3px]">
      {live.map((o) => (
        <button
          key={o}
          aria-pressed={active === o}
          className={
            "whitespace-nowrap rounded-sm px-2.5 py-1 text-[12px] " +
            (active === o
              ? "bg-card text-text shadow-[0_1px_0_rgb(var(--border))]"
              : "text-muted hover:text-text")
          }
        >
          {o}
        </button>
      ))}
      {deferred.map((o) => (
        <button
          key={o}
          aria-disabled
          title={deferredTooltip}
          className="cursor-not-allowed whitespace-nowrap rounded-sm border border-dashed border-border px-2.5 py-1 text-[12px] text-muted/60"
        >
          {o}
        </button>
      ))}
    </div>
  );
}

function IconButton({
  children,
  title,
  onClick,
}: {
  children: React.ReactNode;
  title: string;
  onClick?: () => void;
}) {
  return (
    <button
      title={title}
      aria-label={title}
      onClick={onClick}
      className="inline-flex h-8 w-8 items-center justify-center rounded-sm border border-border text-muted hover:text-text"
    >
      {children}
    </button>
  );
}

function DisabledIconButton({
  children,
  title,
}: {
  children: React.ReactNode;
  title: string;
}) {
  return (
    <button
      title={title}
      aria-label={title}
      aria-disabled
      className="inline-flex h-8 w-8 cursor-not-allowed items-center justify-center rounded-sm border border-dashed border-border text-muted/50"
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
