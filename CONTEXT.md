# noupling

An architecture-auditing CLI: it scans a codebase's import graph, audits it for structural problems, and renders the result in many report formats. This glossary fixes the words those reports share.

## Language

### Findings

**Finding**:
Anything the audit says about a codebase. Every Finding is either an **Issue** or a **Metric**; nothing is both.

**Issue**:
A Finding that names specific modules or edges and asks for a change. The set of Issue kinds is closed (nine today); adding one is a deliberate decision, not a side effect of adding a detector.
_Avoid_: problem, warning, violation (as the umbrella term — "violation" is one Issue kind, see below)

**Metric**:
A Finding that describes a module or directory without asking for a change (fan-in, instability, critical path, external imports, …). Reports may include or omit Metrics freely.
_Avoid_: stat, score (score is the single project-level number, not a per-module Metric)

**Severity band**:
The importance of an Issue on a four-step scale: critical, high, medium, low. Assigned once by the audit; every report shows the same band for the same Issue.
_Avoid_: level, priority, RRI (RRI is the underlying number for the kinds that have one)

**Baseline**:
The set of Issues a team has explicitly accepted. An Issue in the baseline is **baselined**: still reported, marked as accepted, and never counted as "not in baseline" by the CI gate. The baseline covers every Issue kind. Not to be confused with **Violation Age**, whose _new_ bucket means "first seen in the latest snapshot" regardless of the baseline.
_Avoid_: ignored, suppressed (a **suppression** hides an edge at scan time; a baselined Issue is still visible)

**Issue card**:
The canonical presentation of one Issue: kind, severity band, subject, reason, recommendation, score impact, in that order. Every Issue-listing format renders the card in its own medium with the same content.

**Score impact**:
The points one Issue takes off the project score. Zero for kinds that do not score. Summed over all Issues it equals the points lost, so any breakdown adds up.
_Avoid_: contribution, penalty, weight

**Subject**:
What an Issue is about: one module or directory for node-shaped Issues, one `from → to` edge for edge-shaped Issues.

**Reason**:
One sentence saying why this particular Issue exists, with its numbers ("25 imports between analyzer and core, median 5"). Mandatory for every Issue.
_Avoid_: explainer, verdict (the per-kind background prose is a "kind description", not the reason)

**Recommendation**:
One sentence saying what to do about this Issue. Mandatory for every Issue.
_Avoid_: action, fix, suggestion

### Issue kinds

**Coupling Violation**:
An edge between modules whose direction the audit disallows: sibling or upward. A mutually-dependent pair is never a Coupling Violation; it is part of a **Cycle**.

**Cycle**:
An ordered ring of modules that depend on each other back to the start. Its subject is the ring, its reason names the cheapest break edge, its recommendation is to cut it. A ring surfaces exactly once, never also as its constituent edges.
_Avoid_: circular dependency, circular violation (as an Issue kind)

**Rule Violation**:
An edge forbidden by an explicit `dependency_rules` entry in the project's settings.

**Layer Violation**:
An edge from a layered module to a module in a higher layer, or to a module in no layer at all.

**Gravity Well**:
A module whose aggregate relationship risk is disproportionate; everything nearby bends toward it.

**Red Flag**:
A named structural anti-pattern the audit recognises (fused siblings, trapped children, …), carrying a recommendation.

**Stability Violation**:
A more-stable directory depending on a less-stable one (Stable Dependencies Principle).

**Zone Flag**:
A directory far enough off the main sequence to land in the Zone of Pain or the Zone of Uselessness.
_Avoid_: distance (the underlying Metric), danger zone

**Low Cohesion**:
A Package whose direct children barely depend on each other.

### Report formats

**Issue-listing format**:
A report format whose job is to enumerate Findings (text, json, xml, html, md, dashboard, explorer, pr, briefing, sonar). It carries every Issue kind; it may summarise a kind but never silently omit one that has members.

**Graph format**:
A report format that draws the dependency graph (mermaid, dot, bundle). It accents every edge-shaped Issue on the drawing and may omit node-shaped Issues.

**Edge-shaped Issue**:
An Issue about dependency edges: Coupling, Rule, Layer, and Stability Violations, and Cycles (a ring of edges).

**Node-shaped Issue**:
An Issue about a module or directory: Gravity Well, Red Flag, Zone Flag, Low Cohesion.

**Trend format**:
A report format that shows how Issue counts and the score move across snapshots (strategy). It shows counts per Issue kind, not individual Issues.

## Flagged ambiguities

- **"Violation"** is used in the code both for the Coupling Violation kind and as a loose umbrella (the text report's `Violations:` headline). In this glossary it always means one of the four `… Violation` kinds (Coupling, Rule, Layer, Stability); the umbrella is **Issue**. Since 0.9.0 the headline counts Coupling Violations plus Cycles as `issues()` does (`AuditResult::violation_count()`).
- **"Cycle" vs "circular violation"**: resolved in 0.9.0 — the detector still emits ring hops as coupling records (they feed the score), but `issues()` folds them into one Cycle and no format lists them as Coupling Violations.

## Example dialogue

**Dev:** The PR comment format only prints violations. Is that a bug?
**Domain expert:** Yes. It's an Issue-listing format, so it has to carry every Issue kind. It can summarise ("3 Red Flags, 1 Cycle") but it can't drop a kind that has members.
**Dev:** And the fan-in table in the dashboard?
**Domain expert:** That's a Metric. Formats can drop Metrics without anyone calling it inconsistent.
**Dev:** Modules A, B and C import each other in a ring. How many Issues is that?
**Domain expert:** One Cycle. Its subject is the ring, its reason names the cheapest break edge. Nobody also lists A → B as a Coupling Violation.
**Dev:** The team accepted that cycle last quarter.
**Domain expert:** Then it's baselined. It still shows up on every report, marked accepted, and the CI gate doesn't count it against you.
**Dev:** Why does the Explorer say "high" and the text report say "24.0" for the same Red Flag?
**Domain expert:** They shouldn't differ. The band is "high"; 24.0 is the RRI underneath it. Every format shows the same band, and may also print the RRI.
