// Per-kind background prose for the "About this verdict" section in the
// DetailsPanel (#276, reshaped by #345). This is *kind description* only —
// "what a gravity well is" — never per-Issue wording: the reason and
// recommendation for each Issue come from core via the Data Contract's
// `issues` array, so every report format says the same thing.
//
// Hard-coded because the template and analyzer ship in the same release
// and the Explorer must work offline via file://.

import type { IssueKindId } from "./types";

export interface KindDescription {
  title: string;
  /** One-paragraph "what is this and why does it matter" explainer. */
  what: string;
}

export const KIND_DESCRIPTIONS: Record<IssueKindId, KindDescription> = {
  coupling_violation: {
    title: "Why this is a coupling violation",
    what:
      "A sibling or upward import the audit disallows: two directories at the same level (or a child and its parent) " +
      "reaching into each other instead of sharing through a common parent or an interface. Severity is depth-discounted " +
      "— deeper directories matter less because their blast radius is smaller.",
  },
  cycle: {
    title: "Why this is a cycle",
    what:
      "These directories form a directed ring: A depends on B (directly or transitively), B depends back on A. " +
      "Cycles make refactoring hard — you can't change one without affecting the other — and they prevent any " +
      "layered understanding of the codebase. noupling recommends breaking the ring at the hop with the fewest " +
      "imports (the minimum cut), the cheapest place to introduce an abstraction or invert the dependency.",
  },
  rule_violation: {
    title: "Why this is a rule violation",
    what:
      "An import forbidden by an explicit dependency_rules entry in .noupling/settings.json. The team wrote the rule; " +
      "the audit only reports where the code disagrees with it.",
  },
  layer_violation: {
    title: "Why this is a layer violation",
    what:
      "An import from a layered module into a higher layer (or into no layer at all). Layered architectures break down " +
      "when lower-level modules reach upward — what looks like a quick coupling tends to compound into bidirectional " +
      "dependencies and cycles.",
  },
  gravity_well: {
    title: "Why this is a gravity well",
    what:
      "A gravity well is a module that pulls disproportionate aggregate Relationship Risk Index (RRI) — direction " +
      "weight × density. It's the architectural equivalent of a heavy planet: every nearby module bends toward it. " +
      "Wells are not bugs by themselves; they're concentration risk. Refactors that touch the well ripple widely.",
  },
  red_flag: {
    title: "Why this is a red flag",
    what:
      "Red flags are pattern matches the analyzer recognises as architectural anti-patterns — fused siblings, " +
      "trapped children, and similar. They're flagged because the structural cost of unwinding them grows " +
      "non-linearly with codebase age.",
  },
  stability_violation: {
    title: "Why this is a stability violation",
    what:
      "Martin's Stable Dependencies Principle: a more-stable directory (lower instability I) should not depend on a " +
      "less-stable one. When it does, every change in the volatile side forces a change in the stable side, which " +
      "everything else already depends on.",
  },
  zone_flag: {
    title: "Why this directory is in a danger zone",
    what:
      "Distance from the main sequence, D = |A + I − 1|. The Zone of Pain is concrete yet stable — lots of code depends " +
      "on it and it has no abstractions, so every change ripples. The Zone of Uselessness is abstract yet unstable — " +
      "abstractions nobody depends on, speculative architecture that pays no rent.",
  },
  low_cohesion: {
    title: "Why this package has low cohesion",
    what:
      "Cohesion is how much a package's direct children depend on each other. Near zero means the files were grouped " +
      "by convenience, not by responsibility: they change for different reasons and would split cleanly.",
  },
};
