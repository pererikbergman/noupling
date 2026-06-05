// Static per-kind prose for the "About this verdict" section in the
// DetailsPanel (#276). Hard-coded because the template and analyzer
// ship in the same release — version drift risk is minimal, and keeping
// the Data Contract lean was an explicit decision.
//
// Adapt prose from `docs/metrics-guide.md` / reporter-side guide; the
// Explorer must work offline via file://, so no external links here.

export interface VerdictKind {
  title: string;
  /** One-paragraph "what is this and why does it matter" explainer. */
  what: string;
}

export const VIOLATION_EXPLAINER: VerdictKind = {
  title: "Why this is a violation",
  what:
    "A dependency-rule violation means this module imports something its layer or rule policy explicitly forbids. " +
    "Layered architectures break down when lower-level modules reach upward — what looks like a quick coupling " +
    "tends to compound into bidirectional dependencies and cycles. Severity is depth-discounted: deeper " +
    "directories matter less because their blast radius is smaller.",
};

export const CYCLE_EXPLAINER: VerdictKind = {
  title: "Why this is a cycle",
  what:
    "These modules form a directed cycle: A depends on B (directly or transitively), B depends back on A. " +
    "Cycles make refactoring hard — you can't change one without affecting the other — and they prevent any " +
    "layered understanding of the codebase. noupling recommends breaking the cycle at the hop with the fewest " +
    "imports (the minimum cut), since that's the cheapest place to introduce an abstraction or invert " +
    "the dependency.",
};

export const GRAVITY_WELL_EXPLAINER: VerdictKind = {
  title: "Why this is a gravity well",
  what:
    "A gravity well is a module that pulls disproportionate aggregate Relationship Risk Index (RRI) — direction " +
    "weight × density. It's the architectural equivalent of a heavy planet: every nearby module bends toward it. " +
    "Wells are not bugs by themselves; they're concentration risk. Refactors that touch the well ripple widely.",
};

export const RED_FLAG_EXPLAINER: VerdictKind = {
  title: "Why this is a red flag",
  what:
    "Red flags are pattern matches the analyzer recognises as architectural anti-patterns — fused siblings, " +
    "trapped children, and similar. They're flagged because the structural cost of unwinding them grows " +
    "non-linearly with codebase age. Each flag carries a recommendation specific to the pattern.",
};
