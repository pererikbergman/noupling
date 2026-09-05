import { useEffect } from "react";

export interface HelpDialogProps {
  open: boolean;
  onClose: () => void;
}

export function HelpDialog({ open, onClose }: HelpDialogProps) {
  useEffect(() => {
    if (!open) return;
    function onKey(e: KeyboardEvent) {
      if (e.key === "Escape") onClose();
    }
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [open, onClose]);

  if (!open) return null;
  return (
    <div
      role="dialog"
      aria-label="Keyboard shortcuts and help"
      onClick={onClose}
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 p-4"
    >
      <div
        onClick={(e) => e.stopPropagation()}
        className="w-full max-w-md rounded-md border border-border bg-card p-5 text-text shadow-2xl"
      >
        <header className="mb-3 flex items-center justify-between">
          <h2 className="m-0 text-[15px] font-semibold">Keyboard shortcuts</h2>
          <button
            onClick={onClose}
            aria-label="Close help"
            className="rounded-sm px-2 py-1 text-[12px] text-muted hover:text-text"
          >
            ✕ <span className="text-[10px] text-muted/60">esc</span>
          </button>
        </header>
        <dl className="grid grid-cols-[auto_1fr] gap-x-4 gap-y-1.5 text-[13px]">
          <Kbd>Tab</Kbd>
          <dd className="text-muted">Move focus across LSM nodes</dd>
          <Kbd>Enter / Space</Kbd>
          <dd className="text-muted">Open details for the focused node</dd>
          <Kbd>D</Kbd>
          <dd className="text-muted">Drill into the focused node's directory</dd>
          <Kbd>Esc</Kbd>
          <dd className="text-muted">Close details / dialogs</dd>
          <Kbd>← / →</Kbd>
          <dd className="text-muted">(Prototype switcher, dev builds only)</dd>
        </dl>

        <h3 className="mt-5 mb-2 text-[11px] font-semibold uppercase tracking-wider text-muted">
          Toolbar
        </h3>
        <dl className="grid grid-cols-[auto_1fr] gap-x-4 gap-y-1.5 text-[13px]">
          <Kbd>+ / − / ⛶</Kbd>
          <dd className="text-muted">Zoom the LSM canvas in / out / reset</dd>
          <Kbd>L</Kbd>
          <dd className="text-muted">Toggle layer-identity overlay</dd>
          <Kbd>⊚</Kbd>
          <dd className="text-muted">Toggle cycle highlighting</dd>
          <Kbd>↺</Kbd>
          <dd className="text-muted">Reset view to defaults</dd>
          <Kbd>↗</Kbd>
          <dd className="text-muted">Download the raw Data Contract JSON</dd>
        </dl>

        <h3 className="mt-5 mb-2 text-[11px] font-semibold uppercase tracking-wider text-muted">
          Coming later
        </h3>
        <p className="m-0 text-[12px] leading-relaxed text-muted">
          The Matrix / Composition views, path finder (↣), min-cut (⌀),
          Inside / + External scope toggle, hide-by-kind chips, and the
          refactor PLAN sandbox are placeholders for the v2 / v3 milestones in
          PRD §9 + §10. They render with a dashed border and dimmed styling so
          you can tell them apart from wired controls.
        </p>
      </div>
    </div>
  );
}

function Kbd({ children }: { children: React.ReactNode }) {
  return (
    <dt className="font-mono text-[12px] text-text">
      <kbd className="rounded-sm border border-border bg-canvas px-1.5 py-0.5">
        {children}
      </kbd>
    </dt>
  );
}
