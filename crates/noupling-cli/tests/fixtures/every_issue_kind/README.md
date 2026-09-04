# every_issue_kind

Integration fixture for `tests/issue_coverage.rs` (#339). Each directory
exists to make exactly one detector fire; see the comment at the top of
each file. Keep it minimal: adding an import can change medians and
silently un-trigger a neighbour.

| Issue kind            | Where                                    |
|-----------------------|------------------------------------------|
| Coupling Violation    | `loose/x → loose/y` (sibling, weight 1)  |
| Cycle                 | `ring/alpha ↔ ring/beta`                 |
| Rule Violation        | `plugins → legacy` (settings rule)       |
| Layer Violation       | `infra → ui` (infra is the bottom layer) |
| Gravity Well          | `fused/left` (two heavy relationships)   |
| Red Flag              | `fused/left ↔ fused/right` fused sibling |
| Stability Violation   | `stable → volatile`                      |
| Zone of Pain          | `concrete/` (structs only, imported)     |
| Zone of Uselessness   | `abstract_only/` (traits only, imports)  |
| Low Cohesion          | `bag/` (three files, no internal edges)  |
