import type { DataContract } from "../types";

export function SearchRow({ data }: { data: DataContract }) {
  return (
    <div className="col-span-2 flex items-center gap-2 border-b border-border bg-card px-4 py-2">
      <span className="rounded-sm border border-border px-2.5 py-1.5 text-[10px] font-semibold uppercase tracking-wider text-muted">
        Project overview
      </span>
      <div className="flex flex-1 items-center gap-2 rounded-md border border-border bg-canvas px-3 py-1">
        <span className="text-muted">⌕</span>
        <input
          className="flex-1 bg-transparent text-[13px] text-text outline-none placeholder:text-muted"
          placeholder="Search nodes by name, path, or layer…"
        />
        <div className="flex gap-0.5">
          <ModeBtn active>Substring</ModeBtn>
          <ModeBtn>Regex</ModeBtn>
        </div>
      </div>
      <span className="text-[12px] text-muted">
        {data.codebase.module_count} modules · {data.codebase.file_count} files
      </span>
    </div>
  );
}

function ModeBtn({
  active,
  children,
}: {
  active?: boolean;
  children: React.ReactNode;
}) {
  return (
    <button
      className={
        "rounded-sm px-2.5 py-1 text-[11px] " +
        (active ? "bg-pill text-pill-text" : "text-muted hover:text-text")
      }
    >
      {children}
    </button>
  );
}
