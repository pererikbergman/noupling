# Features

## Coupling Violations

A coupling violation occurs when sibling directories depend on each other. If `src/scanner/` imports from `src/core/`, that creates a coupling between two siblings under `src/`.

### Severity

```
severity = weight / (depth + 1)
```

- **weight**: number of import statements between the directory pair
- **depth**: how deep in the directory tree the violation occurs

Root-level coupling (depth 0) is most severe. Deep coupling in leaf packages is mild.

### Weight aggregation

Multiple imports between the same directory pair are aggregated. If `scanner/parser.rs` and `scanner/resolver.rs` both import from `core/`, the weight is 2 with a single violation entry showing `x2`.

## Circular Dependencies

A circular dependency exists when a chain of imports forms a loop: A imports B, B imports C, C imports A.

### Detection

noupling detects circular dependencies among **sibling directories** at each level of the directory tree. It uses D_acc (accumulated external dependencies) computed bottom-up, then DFS cycle finding on the sibling adjacency graph.

### Cycle order

- **Order 2** (mutual): A imports B, B imports A
- **Order 3** (triangular): A -> B -> C -> A
- **Order N**: cycles involving N directories

### Severity

```
severity = total_modules / (depth + 1) / 10
```

Circular dependencies are always significant. The penalty scales with project size and is amplified near the root.

### XS metric

Each circular dependency includes:

- **Weakest link**: the hop in the cycle with the fewest imports — the easiest place to break the cycle
- **Break cost**: number of imports at the weakest link
- **Per-hop XS counts**: visible in the HTML report's expandable cycle details

## Health Score

```
score = 100 * (1 - sum_of_severities / total_modules)
```

| Range | Status |
|:---|:---|
| 90-100 | Healthy (green) |
| 70-89 | Warning (yellow) |
| 0-69 | Needs attention (red) |

## Hotspots

Modules with high fan-in (many incoming imports) are flagged as hotspots. These are architectural bottlenecks — changes to a hotspot affect many dependents.

The default threshold is 10 incoming imports (configurable via `hotspot_fan_in`).

## Cohesion

Per-directory cohesion measures how much files within a directory depend on each other:

```
cohesion = internal_deps / (file_count * (file_count - 1))
```

Low cohesion (default threshold: 0.1) suggests the directory groups unrelated files that might belong in separate packages.

## Inline Suppression

Suppress specific import violations with `noupling:ignore` comments:

```kotlin
import com.example.legacy.Helper // noupling:ignore
```

Or on the line above:

```python
# noupling:ignore
from legacy import helper
```

Supported comment styles: `//` (C-like languages), `#` (Python), `--` (Haskell).

Suppressed imports are excluded from analysis entirely. The suppressed count is shown in scan and audit output.

Disable with `"allow_inline_suppression": false` in settings.

## Diff Mode

Scan the full project but only report violations involving changed files:

```bash
noupling scan . --diff-base main
noupling audit .
```

The scan stores diff metadata in `.noupling/diff-meta.json`. The audit reads it and filters violations to only those touching changed files. Use this in CI to fail PRs only on **new** issues.

## Baseline

For projects with existing violations, save a baseline and only flag new ones:

```bash
# Save current state as accepted
noupling scan .
noupling baseline save .

# Later: only report new violations
noupling scan .
noupling audit . --baseline
```

The baseline uses fingerprints (from/to module paths) to identify which violations are known. Resolved violations are reported separately.

## Supported Languages

| Language | Extensions | Import Pattern |
|:---|:---|:---|
| C# | `.cs` | `using` directives |
| Go | `.go` | `import` declarations |
| Haskell | `.hs` | `import` declarations |
| Java | `.java` | `import` declarations |
| JavaScript | `.js`, `.jsx` | ES `import` statements |
| Kotlin | `.kt`, `.kts` | `import` declarations |
| Python | `.py` | `import` / `from...import` |
| Rust | `.rs` | `use` declarations |
| Swift | `.swift` | `import` declarations |
| TypeScript | `.ts`, `.tsx` | ES `import` statements |
| Zig | `.zig` | `@import()` builtins |

All languages use Tree-sitter for fast, accurate AST-based import extraction.
