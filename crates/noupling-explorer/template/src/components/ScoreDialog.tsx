import { useEffect } from "react";
import type { DataContract, ScoreContributor } from "../types";

export interface ScoreDialogProps {
  data: DataContract;
  open: boolean;
  onClose: () => void;
  onSelect?: (id: string) => void;
}

export function ScoreDialog({ data, open, onClose, onSelect }: ScoreDialogProps) {
  useEffect(() => {
    if (!open) return;
    function onKey(e: KeyboardEvent) {
      if (e.key === "Escape") onClose();
    }
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [open, onClose]);

  if (!open) return null;
  const b = data.score_breakdown;
  return (
    <div
      role="dialog"
      aria-label="Health score breakdown"
      onClick={onClose}
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 p-4"
    >
      <div
        onClick={(e) => e.stopPropagation()}
        className="w-full max-w-lg rounded-md border border-border bg-card p-5 text-text shadow-2xl"
      >
        <header className="mb-3 flex items-center justify-between">
          <h2 className="m-0 text-[15px] font-semibold">
            Health score: {format(data.health_score)}/100
          </h2>
          <button
            onClick={onClose}
            aria-label="Close score breakdown"
            className="rounded-sm px-2 py-1 text-[12px] text-muted hover:text-text"
          >
            ✕ <span className="text-[10px] text-muted/60">esc</span>
          </button>
        </header>

        <section className="mb-4 rounded-sm border border-border bg-canvas p-3">
          <p className="m-0 mb-1 text-[11px] uppercase tracking-wider text-muted">
            Points lost
          </p>
          <p className="m-0 font-mono text-[13px]">
            100 − <strong className="text-accent-domain">{format(data.health_score)}</strong> ={" "}
            <strong data-testid="points-lost">{format(b.points_lost)}</strong> across{" "}
            {b.total_modules} modules
          </p>
          <p className="m-0 mt-1 text-[11px] text-muted">
            Every Issue that scores carries its score impact — the points it takes off.
            Summed over all Issues that equals the points lost, so the rows below add up.
          </p>
        </section>

        {b.points_lost > 0 ? (
          <>
            <h3 className="mt-1 mb-2 text-[11px] font-semibold uppercase tracking-wider text-muted">
              Where the {format(b.points_lost)} lost points come from
            </h3>
            <dl className="mb-4 grid grid-cols-[auto_1fr] gap-x-4 gap-y-1 text-[13px]" data-testid="points-by-kind">
              {b.by_kind.map((k) => (
                <div key={k.kind} className="contents">
                  <dt className="text-muted">{k.kind_name}</dt>
                  <dd className="m-0 font-mono">−{format(k.points)}</dd>
                </div>
              ))}
            </dl>

            {b.top_contributors.length > 0 && (
              <>
                <h3 className="mt-3 mb-2 text-[11px] font-semibold uppercase tracking-wider text-muted">
                  Top contributors
                </h3>
                <ul className="m-0 flex list-none flex-col gap-1 p-0">
                  {b.top_contributors.map((c) => (
                    <ContributorRow
                      key={c.fingerprint}
                      c={c}
                      onClick={() => {
                        onSelect?.(c.focus_id);
                        onClose();
                      }}
                    />
                  ))}
                </ul>
              </>
            )}
          </>
        ) : (
          <p className="m-0 text-[13px] text-muted">
            No deductions — no Issue takes points off this score. Healthy codebase.
          </p>
        )}
      </div>
    </div>
  );
}

function ContributorRow({
  c,
  onClick,
}: {
  c: ScoreContributor;
  onClick: () => void;
}) {
  return (
    <li>
      <button
        onClick={onClick}
        className="block w-full rounded-sm border border-border bg-canvas p-2 text-left text-[12px] transition-colors hover:bg-canvas/60 hover:border-text/30"
        title="Open details for the offender"
      >
        <div className="flex items-center justify-between gap-2">
          <span className="truncate font-mono text-[11px]" title={c.subject}>
            {shortSubject(c.subject)}
          </span>
          <div className="flex shrink-0 items-center gap-1.5">
            <span
              className={
                "rounded-full px-1.5 py-0.5 text-[10px] " +
                (c.kind === "cycle"
                  ? "bg-edge-cycle/20 text-edge-cycle"
                  : "bg-edge-violation/20 text-edge-violation")
              }
            >
              {c.kind_name}
            </span>
            <span className="font-mono text-[10px] text-muted">−{format(c.points)}</span>
          </div>
        </div>
      </button>
    </li>
  );
}

function format(n: number): string {
  const r = Math.round(n * 10) / 10;
  return r.toString();
}

/** `a/b/c.rs -> d/e/f.rs` → `c.rs -> f.rs`; rings likewise, one name per member. */
function shortSubject(subject: string): string {
  return subject
    .split(" -> ")
    .map((p) => p.split("/").filter(Boolean).pop() ?? p)
    .join(" → ");
}
