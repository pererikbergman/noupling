---
status: accepted
date: 2026-09-04
---

# One audit per snapshot; formats never re-audit

Every report format and the `audit` command must read the same `AuditResult` for a given snapshot, so that they all show the same Issues and the same score. Layer auto-detection and the `actionable` coupling-mode fallback, which the Explorer alone applied by re-running the audit in `commands/report.rs`, move into the shared audit pipeline and apply everywhere, including the CI gate. A format is a view over the result and may not change its inputs.

## Considered options

- **Keep inference Explorer-only.** Rejected: a fresh codebase then gets two different Issue sets and two different scores depending on which report you open, which is exactly the inconsistency users notice first.
- **Drop inference entirely.** Rejected: strict mode with no layers scores most unconfigured codebases near zero and produces no Layer Violations, so the first-run experience is a wall of sibling couplings with no signal.
- **Apply inference to reports but keep `audit` strict.** Rejected: a CI gate that disagrees with every report is the same inconsistency one level up.

## Consequences

- Projects with no `layers` in `.noupling/settings.json` will see their score change on upgrade, usually upward, because sibling coupling stops counting once inferred layers switch the audit to actionable mode. This needs a changelog upgrade note and a way to opt out (an explicit `coupling_mode` or `layers` in settings already does).
- The audit result must carry a `layers_auto_detected` flag so any format can say "these layers were inferred" rather than only the Explorer.
- Layer inference moves from the `noupling-explorer` crate to `noupling-core`, since the audit pipeline lives there.
