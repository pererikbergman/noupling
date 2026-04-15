# Monorepo Support

Analyze multiple modules within a single repository independently. Each module gets its own health score, and cross-module dependencies are validated against a declared dependency graph.

## Configuration

Define modules in `.noupling/settings.json`:

```json
{
  "modules": [
    { "name": "app", "path": "app/src", "depends_on": ["lib-core", "lib-network"] },
    { "name": "lib-core", "path": "lib/core/src", "depends_on": [] },
    { "name": "lib-network", "path": "lib/network/src", "depends_on": ["lib-core"] }
  ]
}
```

### Fields

- **`name`**: identifier used in CLI (`--module app`) and reports
- **`path`**: relative path from project root; only files under this path belong to this module
- **`depends_on`**: list of module names this module is allowed to import from

### Dependency graph

`depends_on` defines the allowed cross-module import directions. If module `app` lists `depends_on: ["lib-core"]`, then:

- `app` importing from `lib-core`: allowed
- `app` importing from `lib-network`: **cross-module violation** (not listed)
- `lib-core` importing from `app`: **cross-module violation** (not listed)

## Usage

```bash
# Scan the full project (always scans everything)
noupling scan .

# Multi-module summary
noupling audit .

# Single module detail
noupling audit . --module app

# Report for one module
noupling report . --format html --module lib-core

# Fail CI if overall score drops
noupling audit . --fail-below 80

# Fail CI if a specific module drops
noupling audit . --module app --fail-below 90
```

## Audit output

### Multi-module (no `--module` flag)

```
Overall Score: 82.3/100
Total Modules: 84

MODULE               SCORE    MODULES   VIOLATIONS
app                   75.0         42           12
lib-core              95.0         18            1
lib-network           88.5         24            3

Cross-Module Violations (2):
  app -> lib-network (not in depends_on)
    app/src/main/Api.kt -> lib-network/src/HttpClient.kt (line 5)
  lib-network -> app (not in depends_on)
    lib-network/src/Callback.kt -> app/src/main/AppConfig.kt (line 12)
```

### Single module (`--module app`)

Shows the standard single-module output (violations, hotspots, cohesion) scoped to that module's files.

## How it works

1. **Scanning** always covers the full project. Cross-module imports need the complete module list for resolution.
2. **Analysis** runs `audit()` independently per module: own D_acc computation, BFS coupling detection, and cycle finding.
3. **Cross-module detection** checks every dependency that crosses module boundaries against `depends_on`.
4. **Overall score** is a weighted average of per-module scores by file count, with a penalty for cross-module violations.
5. Files outside all defined module paths are **ignored**.

## Backward compatibility

If `modules` is empty or absent, noupling behaves exactly as before — the entire project is analyzed as a single module. No configuration changes are needed for existing projects.
