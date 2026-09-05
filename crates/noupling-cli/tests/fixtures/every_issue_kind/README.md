# every_issue_kind

Integration fixture for `tests/issue_coverage.rs` (#339, the format-class contract since #350). Each directory
exists to trigger one Issue kind from `CONTEXT.md`; the comment at the top
of each file says which. Some directories fire a second detector as a
side effect (`fused/left` is also low-cohesion, `domain` also lands in the
Zone of Pain) — that's fine, but keep the tree minimal: adding an import
can move a median and silently un-trigger a neighbour.

| Issue kind          | Where                                                                      |
|---------------------|----------------------------------------------------------------------------|
| Coupling Violation  | `loose/x → loose/y` (sibling, weight 1)                                    |
| Cycle               | `ring/alpha ↔ ring/beta`                                                   |
| Rule Violation      | `plugins → legacy` (settings rule)                                         |
| Layer Violation     | `infra → ui` (infra is the bottom layer)                                   |
| Gravity Well        | `fused/left/l1.rs` (carries both of left's relationships)                  |
| Red Flag            | `fused/left ↔ fused/right` fused sibling (6 imports, median 1)             |
| Stability Violation | `stable → volatile`                                                        |
| Zone Flag           | `concrete/` (Zone of Pain) and `abstract_only/` (Zone of Uselessness)      |
| Low Cohesion        | `bag/` (three files, no internal edges)                                    |

Coupling Violation is covered for the `sibling` direction only. The
detector never emits `upward` today, so no fixture can trigger it.
