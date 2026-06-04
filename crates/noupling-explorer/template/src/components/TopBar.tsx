import type { DataContract } from "../types";

export interface TopBarProps {
  data: DataContract;
  theme: "dark" | "light";
  onToggleTheme: () => void;
}

export function TopBar({ data, theme, onToggleTheme }: TopBarProps) {
  const codebaseTitle = basename(data.codebase.path);
  return (
    <div className="col-span-2 flex flex-wrap items-center gap-2.5 border-b border-border bg-card px-4 py-2.5">
      <Logo />
      <Badge>{`v${data.noupling_version}`}</Badge>
      <span className="text-[13px] text-muted ml-1">{codebaseTitle}</span>

      <Divider />

      {/* View modes (the four headline views from the PRD + Structure101) */}
      <PillGroup
        options={["LSM", "Matrix", "Force", "Composition"]}
        active="LSM"
      />

      {/* Scope: inside-the-boundary vs include external */}
      <PillGroup options={["Inside", "+ External"]} active="Inside" />

      {/* Hide-by-kind chips (kind taxonomy is illustrative for now) */}
      <PillGroup
        options={["UI", "Domain", "Infra", "Tests", "Generated"]}
        active={["UI", "Domain", "Infra"]}
      />

      <div className="flex-1" />

      {/* Structure101-style utility icons */}
      <IconButton title="Find dependency path A → B (P)">↣</IconButton>
      <IconButton title="Cut cycles (suggest min-cut)">⌀</IconButton>
      <IconButton title="Filter graph (F)">▼</IconButton>
      <IconButton title="Export (E)">↗</IconButton>
      <IconButton title="Toggle theme" onClick={onToggleTheme}>
        {theme === "dark" ? "☾" : "☼"}
      </IconButton>
      <IconButton title="Keyboard shortcuts (?)">?</IconButton>
    </div>
  );
}

function Logo() {
  return (
    <span className="font-bold tracking-tight text-base">
      <span className="text-accent-domain mr-1.5">◉</span>noupling
    </span>
  );
}

function Badge({ children }: { children: React.ReactNode }) {
  return (
    <span className="inline-flex items-center gap-1.5 rounded-full bg-success px-2.5 py-0.5 text-[11px] font-semibold text-white before:content-[''] before:inline-block before:h-1.5 before:w-1.5 before:rounded-full before:bg-white">
      {children}
    </span>
  );
}

function Divider() {
  return <div className="mx-1 h-5 w-px bg-border" />;
}

function PillGroup({
  options,
  active,
}: {
  options: string[];
  active: string | string[];
}) {
  const activeSet = new Set(Array.isArray(active) ? active : [active]);
  return (
    <div className="flex items-center gap-1 rounded-md border border-border bg-canvas p-[3px]">
      {options.map((o) => (
        <button
          key={o}
          className={
            "whitespace-nowrap rounded-sm px-2.5 py-1 text-[12px] " +
            (activeSet.has(o)
              ? "bg-card text-text shadow-[0_1px_0_rgb(var(--border))]"
              : "text-muted hover:text-text")
          }
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
      onClick={onClick}
      className="inline-flex h-8 w-8 items-center justify-center rounded-sm border border-border text-muted hover:text-text"
    >
      {children}
    </button>
  );
}

function basename(p: string): string {
  return p.split("/").filter(Boolean).pop() ?? p;
}
