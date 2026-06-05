import { useEffect, useState } from "react";

export interface MethodologyDialogProps {
  open: boolean;
  onClose: () => void;
}

type Section =
  | "overview"
  | "views"
  | "concepts"
  | "metrics"
  | "workflow"
  | "glossary";

/**
 * Blocking dialog that explains the principles the Explorer is built
 * on, what each view shows, and what insights to look for. Companion
 * to the keyboard-shortcuts HelpDialog — open via the "Guide" button
 * in the top bar.
 *
 * Hard-coded prose (not enriched from the Data Contract) — the
 * concepts here are stable across codebases, and shipping the
 * explanations in-bundle keeps the Explorer self-contained
 * (PRD G3 — works offline via file://).
 */
export function MethodologyDialog({ open, onClose }: MethodologyDialogProps) {
  const [section, setSection] = useState<Section>("overview");

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
      aria-label="Explorer guide"
      onClick={onClose}
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 p-4"
    >
      <div
        onClick={(e) => e.stopPropagation()}
        className="flex h-[85vh] w-full max-w-4xl flex-col rounded-md border border-border bg-card text-text shadow-2xl"
      >
        <header className="flex items-center justify-between border-b border-border px-5 py-3">
          <h2 className="m-0 text-[16px] font-semibold">
            Explorer guide — how to read this view
          </h2>
          <button
            onClick={onClose}
            aria-label="Close guide"
            className="rounded-sm px-2 py-1 text-[12px] text-muted hover:text-text"
          >
            ✕ <span className="text-[10px] text-muted/60">esc</span>
          </button>
        </header>

        <div className="flex min-h-0 flex-1">
          <nav className="flex w-44 flex-col gap-0.5 border-r border-border bg-canvas/40 p-2">
            <NavButton id="overview" current={section} onClick={setSection}>
              Overview
            </NavButton>
            <NavButton id="views" current={section} onClick={setSection}>
              The four views
            </NavButton>
            <NavButton id="concepts" current={section} onClick={setSection}>
              Concepts
            </NavButton>
            <NavButton id="metrics" current={section} onClick={setSection}>
              Metrics
            </NavButton>
            <NavButton id="workflow" current={section} onClick={setSection}>
              Workflow
            </NavButton>
            <NavButton id="glossary" current={section} onClick={setSection}>
              Glossary
            </NavButton>
          </nav>

          <article className="flex-1 overflow-y-auto px-6 py-4 text-[13px] leading-relaxed">
            {section === "overview" && <Overview />}
            {section === "views" && <Views />}
            {section === "concepts" && <Concepts />}
            {section === "metrics" && <Metrics />}
            {section === "workflow" && <Workflow />}
            {section === "glossary" && <Glossary />}
          </article>
        </div>
      </div>
    </div>
  );
}

function NavButton({
  id,
  current,
  onClick,
  children,
}: {
  id: Section;
  current: Section;
  onClick: (s: Section) => void;
  children: React.ReactNode;
}) {
  const active = id === current;
  return (
    <button
      onClick={() => onClick(id)}
      className={
        "rounded-sm px-3 py-1.5 text-left text-[12px] transition-colors " +
        (active
          ? "bg-canvas/80 text-text"
          : "text-muted hover:bg-canvas/60 hover:text-text")
      }
    >
      {children}
    </button>
  );
}

function H3({ children }: { children: React.ReactNode }) {
  return (
    <h3 className="m-0 mb-1 mt-4 text-[14px] font-semibold first:mt-0">
      {children}
    </h3>
  );
}

function P({ children }: { children: React.ReactNode }) {
  return <p className="m-0 mb-3 text-muted">{children}</p>;
}

function Strong({ children }: { children: React.ReactNode }) {
  return <strong className="text-text">{children}</strong>;
}

function Overview() {
  return (
    <>
      <H3>What the Explorer is</H3>
      <P>
        The Explorer is a <Strong>learning surface</Strong> for a codebase
        noupling has scanned — an interactive readme that lets you build a
        mental model of the project's architecture before you touch a file.
      </P>
      <P>
        Healthy modules are as visible as broken ones. Clean Layers, zero-
        violation packages, and tight clusters get the same canvas real estate
        as cycles and red flags. This is on purpose: the Explorer is meant to
        teach the codebase, not just triage it.
      </P>

      <H3>The two questions it answers</H3>
      <P>
        <Strong>1. What is here?</Strong> Use the Composition view + Files /
        Levels tabs to learn the parts. Use the LSM to see how the parts are
        layered.
      </P>
      <P>
        <Strong>2. What is going wrong?</Strong> Use the Issues tab to walk
        the violations / cycles / gravity wells / red flags. Click any issue
        to focus the canvas on the participants and highlight the offending
        edges at file level.
      </P>

      <H3>Principles</H3>
      <P>
        <Strong>Layered thinking.</Strong> Most architectures have an intended
        direction of dependency: presentation depends on domain, domain
        depends on data, etc. The LSM lays this out top-to-bottom. Edges that
        go <em>up</em> are usually wrong; edges that go <em>across</em>{" "}
        siblings are often a smell.
      </P>
      <P>
        <Strong>Coupling is asymmetric.</Strong> If <code>A</code> imports{" "}
        <code>B</code>, <code>A</code> can't move without <code>B</code>'s
        agreement. <code>B</code> doesn't care. Direction matters; that's why
        noupling tracks <em>which way</em> every edge points.
      </P>
      <P>
        <Strong>Cycles are the headline problem.</Strong> A cycle means there
        is no order in which to read the codebase. Cycles are where
        refactoring hurts most. The Explorer surfaces them on every view.
      </P>
    </>
  );
}

function Views() {
  return (
    <>
      <H3>LSM — Layered Structure Map</H3>
      <P>
        The headline view. Nodes are laid out top-to-bottom by their layer
        (presentation / domain / data / infra). Cross-layer edges are drawn
        between them. Cycle edges are red; rule violations are red-dashed.
      </P>
      <P>
        <Strong>What to look for:</Strong> edges that flow{" "}
        <em>upward</em> between layers, dense sibling coupling within a layer,
        red dashed edges (rule violations), and the cycle badges on individual
        nodes.
      </P>

      <H3>Matrix</H3>
      <P>
        N×N dependency heatmap. Rows are sources, columns are targets. Cell
        intensity = log of edge weight; cycle cells red, violations red-
        dashed.
      </P>
      <P>
        <Strong>What to look for:</Strong> dense vertical bands ("everything
        depends on this module" → likely a god-object or genuine platform
        package), populated cells <em>below</em> the diagonal in a layered
        repo (cross-layer leaks), red blocks.
      </P>

      <H3>Force</H3>
      <P>
        Force-directed layout where tightly coupled nodes pull together. The
        cluster boundaries are pre-computed in Rust via label propagation —
        they show which modules tend to co-import even if your folder
        structure doesn't group them.
      </P>
      <P>
        <Strong>What to look for:</Strong> Clusters that span layer
        boundaries (your folder structure is hiding the real architecture),
        single nodes pulled toward many clusters (genuine cross-cutters), and
        clusters that match a single Layer (healthy locality).
      </P>

      <H3>Composition</H3>
      <P>
        Annotated module map. Each container shows its files, dominant
        language, layer tag, and (when LLM enrichment has been run) a one-
        line natural-language purpose.
      </P>
      <P>
        <Strong>What to look for:</Strong> Modules whose purpose is genuinely
        unclear — they're often the source of cycles and gravity wells.
      </P>
    </>
  );
}

function Concepts() {
  return (
    <>
      <H3>Violation</H3>
      <P>
        A dependency-rule violation means the import goes somewhere the
        module's layer or rule policy forbids. Layered architectures break
        down when lower-level modules reach upward — what looks like a quick
        coupling tends to compound into bidirectional dependencies and
        cycles.
      </P>

      <H3>Cycle</H3>
      <P>
        A directed cycle: <code>A</code> depends on <code>B</code>,{" "}
        <code>B</code> depends back on <code>A</code> (directly or
        transitively). Cycles make refactoring hard — you can't change one
        without affecting the other — and they prevent any layered
        understanding of the codebase.
      </P>
      <P>
        noupling recommends breaking the cycle at the <Strong>minimum cut</Strong>{" "}
        — the hop with the fewest imports, since that's the cheapest place to
        introduce an abstraction or invert the dependency. The Issues tab
        shows this as <code>break: A → B (N vs M)</code>.
      </P>

      <H3>Gravity well</H3>
      <P>
        A module that pulls disproportionate aggregate{" "}
        <Strong>Relationship Risk Index (RRI)</Strong>. Think of it as the
        architectural equivalent of a heavy planet: every nearby module bends
        toward it. Wells are not bugs by themselves — they are
        <em> concentration risk</em>. Refactors that touch the well ripple
        widely.
      </P>

      <H3>Red flag</H3>
      <P>
        Pattern matches the analyzer recognises as architectural anti-
        patterns — fused siblings, trapped children, and similar. They are
        flagged because the structural cost of unwinding them grows non-
        linearly with codebase age. Each flag carries a recommendation
        specific to the pattern.
      </P>
    </>
  );
}

function Metrics() {
  return (
    <>
      <H3>Health score</H3>
      <P>
        <code>100 × (1 − Σ severity / total_modules)</code>. Each violation
        contributes a severity weight (depth-discounted: deeper folders
        matter less). The score is opaque on its own, so the breakdown dialog
        spells out the math, the cycles-vs-coupling split, and the top
        contributors — click the score block to see it.
      </P>

      <H3>RRI — Relationship Risk Index</H3>
      <P>
        <code>direction_weight × density</code>. Captures both how
        architecturally wrong a relationship is (upward / sibling / circular
        all weight higher than downward) and how heavy it is (imports per
        eligible pair). Gravity wells are nodes whose aggregate RRI sits
        above the codebase median.
      </P>

      <H3>Instability (I)</H3>
      <P>
        Robert Martin's <code>I = Ce / (Ca + Ce)</code>. Ranges 0..1. 0 means
        nothing depends on this module's stability (it can change freely); 1
        means it depends on everything and depends on nothing else. Healthy
        codebases have low-I modules at the foundation and high-I modules at
        the edges.
      </P>

      <H3>Afferent (Ca) / Efferent (Ce)</H3>
      <P>
        <Strong>Ca:</Strong> incoming dependencies — how many other modules
        rely on this one. <Strong>Ce:</Strong> outgoing dependencies — how
        many other modules this one relies on.
      </P>

      <H3>Cohesion</H3>
      <P>
        <code>internal_deps / (n_children × (n_children − 1))</code>. How much
        of a package's children depend on each other vs. on outside code.
        High cohesion = a tight module that belongs together. Low cohesion =
        a folder of unrelated files.
      </P>

      <H3>Blast radius / LOC</H3>
      <P>
        <Strong>Blast radius</Strong> = transitive number of modules
        reachable from this file. A 1-line file at the foundation can have a
        blast radius of thousands. <Strong>LOC</Strong> = lines of code.
      </P>
    </>
  );
}

function Workflow() {
  return (
    <>
      <H3>First time on a codebase</H3>
      <P>
        1. Read the welcome card in the Info tab and look at the health
        score. Click it to see the breakdown.
      </P>
      <P>
        2. Open the LSM at root scope. The codebase's layers should be visible
        as horizontal bands. If the bands are missing, layers haven't been
        configured (or weren't auto-detected); the Auto-Layers banner in the
        Info tab offers a settings.json snippet.
      </P>
      <P>
        3. Switch to the Composition view to see what each module does at a
        glance. Pair with the Levels tab for one-level-at-a-time browsing.
      </P>

      <H3>Investigating a problem</H3>
      <P>
        1. Open the Issues tab. Issues are sorted by severity: high
        violations, cycles, gravity wells, red flags, then medium / low
        violations.
      </P>
      <P>
        2. Click an issue. The canvas drills to the lowest common ancestor of
        the participants and expands the participant containers to file
        level. The offending edges render in red.
      </P>
      <P>
        3. Press <code>Esc</code> to exit focus mode. The selection stays
        sticky on the issue until you pick another.
      </P>

      <H3>Comparing two snapshots</H3>
      <P>
        Run <code>noupling scan</code> + <code>noupling report --format
        explorer</code> repeatedly over time and the history sparkline in the
        Info tab will show the trend. Hover a dot to see that snapshot's
        score + delta vs. the previous.
      </P>
    </>
  );
}

function Glossary() {
  return (
    <>
      <H3>Quick reference</H3>
      <dl className="grid grid-cols-[160px_1fr] gap-x-4 gap-y-1.5 text-[12px]">
        <dt className="text-text">Layer</dt>
        <dd className="m-0 text-muted">
          A horizontal band in the codebase's intended architecture (e.g.
          presentation, domain, data, infra).
        </dd>
        <dt className="text-text">Scope</dt>
        <dd className="m-0 text-muted">
          The currently drilled-into part of the codebase. Shown in the
          breadcrumb; shared across all views and tabs.
        </dd>
        <dt className="text-text">Container</dt>
        <dd className="m-0 text-muted">
          A directory that holds only sub-directories. Has no files of its
          own; its metrics are aggregated from its descendants.
        </dd>
        <dt className="text-text">Package</dt>
        <dd className="m-0 text-muted">
          A directory that holds at least one file directly. Carries its own
          metrics (file count, cohesion).
        </dd>
        <dt className="text-text">Singleton chain</dt>
        <dd className="m-0 text-muted">
          A run of directories where each has exactly one child. Collapsed
          on the LSM so the user sees <code>com/foo/bar/baz</code> as a
          single card.
        </dd>
        <dt className="text-text">Aggregated edge</dt>
        <dd className="m-0 text-muted">
          When the canvas shows containers, file-level imports between their
          descendants are rolled up into one container-to-container edge.
          Click the edge to see the file-level contributors.
        </dd>
        <dt className="text-text">Spot filter</dt>
        <dd className="m-0 text-muted">
          The pill row above the canvas — filters which nodes are visible
          (in cycles / with violations / clean / gravity wells / hide
          violations).
        </dd>
        <dt className="text-text">Minimum cut</dt>
        <dd className="m-0 text-muted">
          The cheapest edge to break to resolve a cycle, in imports
          required. Shown as <code>break: A → B (N vs M)</code> on the cycle
          issue.
        </dd>
      </dl>
    </>
  );
}
