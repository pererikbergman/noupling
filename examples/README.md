# Examples

Three tiny Kotlin projects, each built to trigger one specific outcome so you
can see what a noupling audit looks like before pointing it at real code. Run
any of them from the repository root:

```bash
noupling scan examples/kotlin-coupled
noupling audit examples/kotlin-coupled
```

Each scan writes its own `examples/<name>/.noupling/` (gitignored).

| Example | What it demonstrates | Health | Issues that score |
| --- | --- | --- | --- |
| `kotlin-clean` | Three packages (`data`, `domain`, `ui`) that never import each other. Nothing to report. | 100.0 | none |
| `kotlin-coupled` | `auth` imports its sibling `billing` twice, in one direction only. A **Coupling Violation** and no Cycle. | 86.7 | 1 Coupling Violation |
| `kotlin-circular` | `orders → inventory → shipping → orders`. A three-package **Cycle**, with the cheapest edge to cut named as the weakest link. | 50.0 | 1 Cycle |

The audits also list non-scoring Issues where the graph earns them (a Zone Flag
on `billing`, Low Cohesion in a package whose files don't use each other). Those
are real findings on these toy projects, not noise: the Issue card says why.

## Keeping them honest

`crates/noupling-cli/tests/cli.rs` (`examples_demonstrate_their_documented_issue_kinds`)
scans a scratch copy of each example and asserts the Issue kinds above, so a
change to the sources or the analyzer that turns `kotlin-coupled` back into a
Cycle fails CI.

## What to try next

- `noupling report examples/kotlin-circular --format explorer` and open
  `examples/kotlin-circular/.noupling/explorer.html`: the ring is drawn and the
  Issue card links to the hop to cut.
- `noupling baseline save examples/kotlin-coupled` followed by
  `noupling audit examples/kotlin-coupled --baseline`: the Coupling Violation is
  still listed, marked `(baselined)`, and the exit code is 0.
- The [How-to](https://pererikbergman.github.io/noupling/howto.html) page has
  the CI, baseline and configuration recipes.
