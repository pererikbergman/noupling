---
status: accepted
date: 2026-09-04
---

# One `issues` array shared by the JSON report and the Explorer contract, replacing the per-kind arrays

The JSON report (`json`, `xml`, `sonar`) and the Explorer's Data Contract were two hand-built serialisations of the same audit and had drifted: rule and layer violations reached the Explorer but never JSON, and severity existed in one and not the other. We serialise the core `Issue` enum once, as an `issues` array of Issue cards (kind, severity band, subject, reason, recommendation, `baselined`, plus per-kind payload), and embed that same array in both. The old per-kind arrays in the JSON report (`coupling_violations`, `circular_dependencies`, `gravity_wells`, `red_flags`, `stability_violations`, `distance`, `cohesion`) are removed in the same release rather than deprecated for a cycle, because the project is pre-1.0 and carrying two shapes for one release would let consumers keep reading the one that can drift.

## Consequences

- Downstream JSON consumers must switch to `issues` and filter by `kind`. Called out as a breaking change in the changelog.
- Metric arrays (`hotspots`, `abstractness`, `instability`, `directory_tree`) stay as they are; only Issue-bearing arrays are replaced.
- The Explorer's `format_version` bumps, and the template reads Issues from `issues` rather than from `violations` / `cycles` / `gravity_wells` / `red_flags` separately.
