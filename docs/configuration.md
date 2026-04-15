# Configuration

All settings are stored in `.noupling/settings.json`, auto-created on first run.

## Full Reference

```json
{
  "thresholds": {
    "score_green": 90.0,
    "score_yellow": 70.0,
    "critical_severity": 0.5,
    "minimum_severity": 0.2,
    "hotspot_fan_in": 10,
    "min_cohesion": 0.1
  },
  "ignore_patterns": [
    "**/.git/**",
    "**/build/**",
    "**/node_modules/**"
  ],
  "source_extensions": [
    "rs", "kt", "java", "ts", "py", "swift", "cs",
    "go", "hs", "js", "jsx", "kts", "tsx", "zig"
  ],
  "layers": [],
  "dependency_rules": [],
  "modules": [],
  "allow_inline_suppression": true
}
```

## Thresholds

| Field | Default | Description |
|:---|:---|:---|
| `score_green` | 90.0 | Scores at or above this are "healthy" (green in reports). |
| `score_yellow` | 70.0 | Scores at or above this are "warning" (yellow). Below is red. |
| `critical_severity` | 0.5 | Violations with severity above this are flagged as critical. |
| `minimum_severity` | 0.2 | Violations below this severity are hidden (reduces noise). |
| `hotspot_fan_in` | 10 | Modules with more incoming imports than this are flagged as hotspots. |
| `min_cohesion` | 0.1 | Directories with cohesion below this are flagged as low-cohesion. |

## Ignore Patterns

Glob patterns (gitignore-style) for directories and files to skip during scanning.

```json
{
  "ignore_patterns": [
    "**/.git/**",
    "**/target/**",
    "**/node_modules/**",
    "**/build/**",
    "**/dist/**",
    "**/generated/**",
    "**/__pycache__/**",
    "**/.venv/**"
  ]
}
```

Patterns use `**` for recursive matching. Any file matching a pattern is excluded from analysis.

## Source Extensions

File extensions to include in the scan. Only files with these extensions are parsed.

```json
{
  "source_extensions": ["kt", "java", "ts", "tsx"]
}
```

## Architectural Layers

Define layers to suppress coupling violations that follow the intended dependency direction. Dependencies may only flow **downward** (from earlier to later in the list).

```json
{
  "layers": [
    { "name": "presentation", "pattern": "**/ui/**" },
    { "name": "domain", "pattern": "**/domain/**" },
    { "name": "data", "pattern": "**/data/**" }
  ]
}
```

- `presentation -> domain`: allowed (downward), not flagged
- `domain -> data`: allowed (downward), not flagged
- `data -> presentation`: violation (upward), flagged as both coupling and layer violation

### Pattern syntax

Layer patterns use glob syntax matching against the full relative file path:

- `**/ui/**` matches any file under a `ui/` directory at any depth
- `src/presentation/**` matches files specifically under `src/presentation/`

## Custom Dependency Rules

Forbid or allow specific dependency patterns between modules.

```json
{
  "dependency_rules": [
    {
      "from": "**/ui/**",
      "to": "**/data/**",
      "allow": false,
      "message": "UI must not depend on data layer directly"
    },
    {
      "from": "**/test/**",
      "to": "**/internal/**",
      "allow": true,
      "message": "Tests may access internal modules"
    }
  ]
}
```

| Field | Description |
|:---|:---|
| `from` | Glob pattern matching the source file path. |
| `to` | Glob pattern matching the target file path. |
| `allow` | `false` = this dependency is forbidden. `true` = explicitly allowed. |
| `message` | Custom message shown when the rule is violated. |

## Monorepo Modules

Define independent modules within a monorepo. Each module gets its own health score and analysis.

```json
{
  "modules": [
    { "name": "app", "path": "app/src", "depends_on": ["lib-core"] },
    { "name": "lib-core", "path": "lib/core/src", "depends_on": [] },
    { "name": "lib-network", "path": "lib/network/src", "depends_on": ["lib-core"] }
  ]
}
```

| Field | Description |
|:---|:---|
| `name` | Human-readable identifier, used in `--module` flag and reports. |
| `path` | Relative path from project root. Only files under this path belong to this module. |
| `depends_on` | Module names this module may import from. Cross-module imports to unlisted modules are violations. |

If `modules` is empty or absent, the entire project is analyzed as a single module.

See [Monorepo Support](monorepo.md) for detailed usage.

## Inline Suppression

When `allow_inline_suppression` is `true` (default), you can suppress individual imports with a `noupling:ignore` comment:

```kotlin
import com.example.data.Repository // noupling:ignore
```

```python
# noupling:ignore
import data.repository
```

```haskell
-- noupling:ignore
import Data.Internal.Module
```

Set `"allow_inline_suppression": false` to disable this feature and enforce that all violations are addressed.
