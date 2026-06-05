import type React from "react";

export function basename(p: string): string {
  return p.split("/").filter(Boolean).pop() ?? p;
}

export function formatScore(n: number): string {
  // Round to 1 decimal; trim trailing zero so 100.0 → 100.
  const r = Math.round(n * 10) / 10;
  return Number.isInteger(r) ? String(r) : r.toFixed(1);
}

export function layerAccent(layer: string | null): string {
  if (!layer) return "bg-muted/30";
  if (layer.includes("ui")) return "bg-accent-ui";
  if (layer.includes("infra")) return "bg-accent-infra";
  return "bg-accent-domain";
}

export function StatRow({
  label,
  children,
}: {
  label: string;
  children: React.ReactNode;
}) {
  return (
    <div className="flex justify-between text-[13px]">
      <span className="text-muted">{label}</span>
      <span>{children}</span>
    </div>
  );
}

export function SectionHeading({ children }: { children: React.ReactNode }) {
  return (
    <h3 className="mb-1.5 mt-4 text-[11px] font-semibold uppercase tracking-wider text-muted">
      {children}
    </h3>
  );
}

export function ScoreButton({
  score,
  onClick,
}: {
  score: number;
  onClick?: () => void;
}) {
  if (!onClick) {
    return (
      <strong className="text-accent-domain">{formatScore(score)}/100</strong>
    );
  }
  return (
    <button
      onClick={onClick}
      aria-label="Show health score breakdown"
      className="cursor-pointer rounded-sm border-b border-dotted border-accent-domain/60 px-0.5 font-bold text-accent-domain hover:bg-accent-domain/10"
      title="Why this score? Click for the breakdown."
    >
      {formatScore(score)}/100
    </button>
  );
}
