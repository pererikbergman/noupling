# Dependency Graph Foundation

How raw source files become a dependency graph in noupling.

## What this doc covers

noupling has two halves: **build the graph**, then **analyze it**. Every metric, every report format, every layer rule consumes the same `(Vec<Module>, Vec<Dependency>)` pair the scanner produces. This document covers only the first half.

After reading it you should be able to:

1. Name the two types that make up the graph and the file they live in.
2. Trace any source-file-to-graph-node path: which filter applied, what got parsed, which resolution branch hit.
3. Add a new language to the producer.
4. Re-implement an equivalent producer in another tool if you wanted to.

What it **does not** cover (each has a one-line pointer in § What's NOT in the graph below):

- Layers, layer violations, layer-based weights
- Metrics (cohesion, hotspots, abstractness, instability, distance, gravity wells, red flags, blast radius)
- The audit pipeline and risk-weighted scoring
- Reporters (text, JSON, HTML, markdown, dashboards, graph formats)
- Diff mode, baseline comparison
- Monorepo cross-module violation detection

Those operate on the graph. They are not part of it.

---

## The two output shapes

The whole graph is two types. Both live in `src/core/mod.rs`. Everything downstream operates on them.

### `Module` — a node

```rust
pub struct Module {
    pub id: String,                  // UUID v4, assigned at discovery
    pub snapshot_id: String,         // temporal grouping key (see Persistence)
    pub parent_id: Option<String>,   // unused in current flat structure
    pub name: String,                // file name, e.g. "main.rs"
    pub path: String,                // relative path, forward-slash normalized
    pub module_type: ModuleType,     // always File for produced nodes
    pub depth: i32,                  // number of path components from root
}
```

| Field | Origin | Why it exists |
|---|---|---|
| `id` | `uuid::Uuid::new_v4()` at discovery time | Stable handle that survives across snapshots in SQLite; lets dependencies reference modules without storing paths as foreign keys |
| `snapshot_id` | Passed in by the caller | Lets the same project produce multiple graph snapshots over time. Two snapshots with identical structure will have distinct module IDs |
| `parent_id` | Always `None` today | Hold for a future hierarchical model. The graph is flat right now |
| `name` | File name from `DirEntry` | Convenience for reporters; redundant with `path` |
| `path` | Path relative to project root, slashes normalized | The primary stable identifier from a *content* perspective. Two scans of an unchanged file produce the same `path` and different `id`s |
| `module_type` | Hard-coded `ModuleType::File` for discovered modules | Holdover from an earlier design that produced `Dir` nodes too. The scanner now produces only files |
| `depth` | `rel_path.components().count()` | Convenience for analyzers that walk the directory tree |

### `Dependency` — an edge

```rust
pub struct Dependency {
    pub from_module_id: String,  // the file containing the `import` statement
    pub to_module_id: String,    // the file being imported
    pub line_number: i32,        // line of the import statement (1-based)
}
```

| Field | Origin | Why it exists |
|---|---|---|
| `from_module_id` | `Module.id` of the file being scanned | Source of the import |
| `to_module_id` | `Module.id` of the resolved target | Target of the import |
| `line_number` | Captured by the language parser at parse time | Lets reporters cite the exact line; lets `noupling:ignore` work |

A `Dependency` is unconditionally **directional** (`from → to`) and **resolved** — it only exists when the parser's resolver mapped the import string to a file that's actually in the discovered module set. Unresolved imports and self-references never become `Dependency` rows; they take different paths (see § Stage 3).

### `Snapshot` — the temporal wrapper

```rust
pub struct Snapshot {
    pub id: String,         // UUID v4
    pub timestamp: String,  // SQLite CURRENT_TIMESTAMP at scan time
    pub root_path: String,  // absolute project root that was scanned
}
```

Important to the *persistence* story but not part of the graph itself. The producer pipeline takes a `snapshot_id` as an input parameter and stamps it on every Module it creates. The graph the producer returns is one snapshot. Snapshots aren't linked into a chain; trend analysis happens by querying the SQLite history.

---

## The producer pipeline

The entry point is one function:

```rust
pub fn scan_project(
    root: &Path,
    snapshot_id: &str,
    allow_inline_suppression: bool,
) -> Result<ScanResult>
```

It runs four stages in order:

```
+-----------+    +-----------+    +-----------+    +-------------+
| Discovery |───▶|   Parse   |───▶|  Resolve  |───▶| Synthesize  |───▶ ScanResult
+-----------+    +-----------+    +-----------+    +-------------+
 src/scanner/    src/scanner/     src/scanner/     src/scanner/
 discovery.rs    parsers/<lang>   parsers/<lang>   mod.rs
                 .rs::parse       .rs::resolve     scan_project
```

| Stage | Input | Output |
|---|---|---|
| Discovery | project root path, settings | `Vec<Module>` (no deps yet) |
| Parse | source text of each module | `Vec<ImportEntry>` per module |
| Resolve | each `ImportEntry` + the set of known paths | `Option<String>` resolved path |
| Synthesize | resolutions + suppression rules + self-edge guard | `Vec<Dependency>` and counters |

The pipeline returns `ScanResult`:

```rust
pub struct ScanResult {
    pub modules: Vec<Module>,                       // the graph nodes
    pub dependencies: Vec<Dependency>,              // the graph edges
    pub suppressed_count: usize,                    // imports dropped by `noupling:ignore`
    pub external_imports: Vec<ExternalImportCount>, // per-module count of unresolved imports
}

pub struct ExternalImportCount {
    pub module_path: String,
    pub count: usize,
}
```

The first two fields *are* the graph. The other two are side-channels carrying counters that downstream analysis cares about but that aren't graph nodes or edges.

---

## Stage 1: Discovery

`src/scanner/discovery.rs::discover_files`. Walks the filesystem, filters, and emits `Module` records. No source file is read in this stage — only the directory tree.

### What it does

```text
load Settings (or defaults if .noupling/settings.json is absent)
compile ignore_patterns into a GlobSet
canonicalize root
walk root recursively, depth-first, deterministically sorted by file name:
    for each directory:
        skip if its relative path (or `<rel>/x`) matches ignore_set
        else recurse
    for each file:
        skip if relative path matches ignore_set
        skip if extension is not in settings.source_extensions
        emit Module {
            id:          UUID v4,
            snapshot_id: <passed-in argument>,
            parent_id:   None,
            name:        file.file_name(),
            path:        slash-normalized relative path,
            module_type: File,
            depth:       rel_path.components().count(),
        }
```

### Filtering rules

- **`source_extensions`** (`Settings.source_extensions`, list of bare extensions like `"rs"`, `"py"`, `"ts"`). A file's extension must appear in this list; otherwise it's skipped. Discovery does not consult the parser registry — it relies on settings alone. A project can configure `source_extensions` to include extensions that have no adapter; those files become Modules with no outgoing edges (because Stage 2 silently skips files whose extension isn't in `parsers::registry()`).
- **`ignore_patterns`** (`Settings.ignore_patterns`, list of glob strings like `"**/target/**"`). Matched as a `globset::GlobSet`. Two forms are checked for directories — the bare relative path and `<rel>/x` — to handle patterns like `**/build/**` that match path *components* rather than directory names directly. File-level matches use just the relative path.

### What discovery does NOT do

- Does not open source files. No I/O on file contents.
- Does not know about language semantics. Extension is the only language-relevant input.
- Does not produce `Dependency` rows. Only `Module`s.
- Does not emit directory nodes — `module_type` is always `File`. The `ModuleType::Dir` variant exists for legacy reasons but isn't produced.
- Does not deduplicate. The filesystem walk doesn't follow symlinks into cycles, but if your settings somehow include the same file twice, you'll get two Modules with different `id`s.

### Output guarantee

Discovery returns `Vec<Module>` in deterministic order (sorted by `DirEntry::file_name` at each level). Two runs over the same unchanged filesystem and the same settings produce the same paths in the same order. Only the `id` field differs (UUIDs are fresh each run).

---

## Stage 2: Per-language parsing & resolution

Each supported language lives in `src/scanner/parsers/<lang>.rs` and implements the `LanguageParser` trait declared in `src/scanner/parsers/mod.rs`. The `registry()` function maps file extensions to adapters.

### The trait

```rust
pub trait LanguageParser: Send + Sync {
    fn parse(&self, source: &str) -> Vec<ImportEntry>;

    fn resolve(
        &self,
        import_path: &str,
        source_file: &str,
        known_paths: &[String],
    ) -> Option<String>;

    fn count_type_declarations(&self, _source: &str) -> TypeCounts {
        TypeCounts::default()
    }
}
```

#### Contract

- **`parse(source)`** returns every import statement in the source, in source order, as `ImportEntry { path, line_number }`. `path` is the raw text the language uses to refer to the import (`"std::collections::HashMap"`, `"./helpers"`, `"com.example.Foo"`, `"./types"`, etc.). `line_number` is 1-based.
- **`resolve(import_path, source_file, known_paths)`** is a pure function that maps the parsed `import_path` to a project-relative file path — or returns `None` if the import refers to something outside the project (stdlib, third-party, missing file). `source_file` is needed for relative imports (Python `.`, Rust `crate::`, TypeScript `./`, Zig `utils.zig`). `known_paths` is the universe of paths discovery produced; the resolver may only return strings that appear in this slice.
- **`count_type_declarations(source)`** counts `trait` / `interface` / `abstract class` declarations vs concrete `struct` / `enum` / `class` declarations. **Not used by the graph.** It feeds the abstractness metric in `analyzer::compute_abstractness` and is mentioned here only so language authors know the method exists. The default impl returns zeros, and 12 of the 16 adapters use that default.

Both `parse` and `resolve` are pure functions — no mutable state, no I/O. The scanner reads the file once and hands the source text to `parse`; the resolver is called per import entry with no further reads.

### `ImportEntry`

```rust
pub struct ImportEntry {
    pub path: String,        // the raw import text from the source
    pub line_number: i32,    // 1-based
}
```

Examples by language:

| Language | Source statement | `ImportEntry.path` |
|---|---|---|
| Rust | `use crate::scanner::parsers;` | `"crate::scanner::parsers"` |
| Python | `from .helpers import compute` | `".helpers"` |
| Python | `import re` | `"re"` |
| TypeScript | `import { x } from "./foo";` | `"./foo"` |
| Java | `import com.example.Foo;` | `"com.example.Foo"` |
| Go | `import "github.com/x/y"` | `"github.com/x/y"` |
| Dart | `import 'package:flutter/material.dart';` | `"package:flutter/material.dart"` |

The parser doesn't try to interpret the import. It just lifts the text out. Interpretation is the resolver's job.

### Why `parse` and `resolve` are split

Splitting the two methods keeps each side dumb. The parser is a tree-sitter walker that emits whatever the grammar says is an import. The resolver is a path-mapping problem that doesn't need to know how AST nodes work. Either side can be tested in isolation: parser tests assert on extracted strings, resolver tests assert on `(input, known_paths) → Option<output>` triples.

A resolver that thought about ASTs would have to re-walk source. A parser that tried to resolve would need filesystem access. The split lets the scanner cache the parsed result if it ever wanted to, and lets test authors poke at each behaviour independently.

### The `registry()`

```rust
pub fn registry() -> Vec<(&'static str, Box<dyn LanguageParser>)> {
    vec![
        ("rs",    Box::new(rust::RustParser)),
        ("kt",    Box::new(kotlin::KotlinParser)),
        ("kts",   Box::new(kotlin::KotlinParser)),
        ("ts",    Box::new(typescript::TypeScriptParser)),
        ("tsx",   Box::new(typescript::TsxParser)),
        ("swift", Box::new(swift::SwiftParser)),
        // ... 15 more extension → adapter pairs
    ]
}
```

The registry has 21 entries spanning 16 adapter types. Languages that share an adapter (e.g. `.js` and `.jsx` both use `JavaScriptParser`) appear as separate entries pointing to separate boxes. This is the *only* place file extensions are mapped to behaviour. Files whose extension isn't in the registry are silently skipped by Stage 2 — they remain in the graph as nodes with no outgoing edges. (Whether they appear as nodes at all depends on Stage 1's `source_extensions`, which is set in settings.)

### `ends_with_segment` — the resolver primitive

Every resolver eventually has to answer "given this candidate path like `pkg/foo.py`, does it match one of the known paths?" The naïve answer is `path.ends_with(candidate)`. That has a sharp edge:

```rust
"src/wave2md/stages/structure.py".ends_with("re.py")  // true!
```

Because byte-suffix matching doesn't respect path component boundaries, `import re` in Python could be resolved against `structure.py` itself (or `figure.py`, or `chaos.py` for `import os`). This bug was real, lived in 12 of the 16 parsers, and shipped silently for several versions before being caught and fixed in [#212 / PR #214](https://github.com/pererikbergman/noupling/pull/214). Every fan-in count and hotspot ranking in those affected resolvers was wrong.

The fix is a one-line helper in `src/scanner/parsers/mod.rs`:

```rust
pub fn ends_with_segment(path: &str, candidate: &str) -> bool {
    path == candidate || path.ends_with(&format!("/{}", candidate))
}
```

It anchors the suffix on a `/` boundary. `"structure.py"` does NOT end with `"/re.py"`, so the spurious match disappears. **Every resolver in this codebase uses this helper.** New language adapters should too — never reach for bare `ends_with` on path candidates.

### Per-language resolver notes

The 16 adapters differ in how they translate `import_path` into candidate file paths. The patterns:

- **Relative-import languages** consume `source_file` to anchor a relative path. Rust (`crate::`, `super::`, `self::`), Python (`.foo`, `..bar`), TypeScript (`./foo`, `../bar`), Dart (`./` package-relative), Zig (`utils.zig`), Swift (relative-by-name).
- **Package-style languages** convert dotted package paths to slash paths. Java (`com.example.Foo` → `com/example/Foo.java`), Kotlin (same), C# (`MyApp.Data` → `MyApp/Data.cs`), Scala (`com.example.Bar` → `com/example/Bar.scala`), Elixir (`MyApp.Bar` → `my_app/bar.ex`, CamelCase → snake_case).
- **Module-name languages** look for files matching the bare import name. Haskell (`Data.List` → `Data/List.hs`), Go (`github.com/x/y` → matches paths containing `/y/`), Ruby (`require_relative './foo'`), PHP (`require './foo.php'`).

What "external" means is language-specific. For Python, an import resolves to `None` if the resolver can't find a matching `.py` file in `known_paths` — so stdlib (`re`, `os`) and third-party (`numpy`) both return `None`. For Rust, an import resolves to `None` if it doesn't start with `crate::`, `super::`, or `self::` (so `std::collections::HashMap` is external from the resolver's standpoint, but so is `serde::Deserialize`). The resolver doesn't try to distinguish stdlib from third-party — that's not part of the graph contract.

---

## Stage 3: Synthesis in `scan_project`

`src/scanner/mod.rs::scan_project` ties everything together. It calls discovery, then for each module reads the source, parses, resolves each import, and decides whether each resolution becomes a `Dependency` row.

### The edge-decision flow

For each `ImportEntry` produced by the parser, four outcomes are possible:

```
ImportEntry
    │
    ▼
┌─────────────────────────────────┐
│ allow_inline_suppression flag   │
│ AND `noupling:ignore` matches?  │── yes ──▶ DROP   ───▶ ++suppressed_count
└────────────────┬────────────────┘
                 │ no
                 ▼
┌─────────────────────────────────┐
│ adapter.resolve(...)            │
└──────┬──────────────────┬───────┘
       │ Some(path)       │ None
       ▼                  ▼
┌──────────────┐    ┌──────────────────────┐
│ to_module    │    │ DROP                 │
│ found?       │    │ ++external_count     │
└──┬────────┬──┘    │ (for this module)    │
   │ yes    │ no    └──────────────────────┘
   ▼        ▼
┌────────────────┐
│ from_id ==     │
│ to_id?         │── yes ──▶ DROP (self-edge guard)
└──────┬─────────┘
       │ no
       ▼
   EMIT Dependency
```

Four exit points: emit (one path), drop (three paths). Two of the drops update side-channel counters; the self-edge drop just throws the edge away.

### The synthesis loop, in pseudocode

```text
let known_paths = modules.map(|m| m.path)        // for the resolver
let ext_map    = build extension → adapter map from registry()

per_file_results = modules.par_iter().filter_map(|module| {
    let adapter = ext_map[module.path.extension()]?;   // skip if no adapter
    let source  = read_to_string(root.join(module.path))?;
    let imports = adapter.parse(&source);

    let mut suppressed = 0;
    let mut external = 0;
    let deps = imports.filter_map(|entry| {
        if allow_inline_suppression && is_suppressed(source, entry.line_number) {
            suppressed += 1;
            return None;
        }
        match adapter.resolve(&entry.path, &module.path, &known_paths) {
            Some(resolved) => {
                let to = modules.iter().find(|m| m.path == resolved)?;
                if to.id == module.id { return None; }   // self-edge guard
                Some(Dependency {
                    from_module_id: module.id.clone(),
                    to_module_id: to.id.clone(),
                    line_number: entry.line_number,
                })
            }
            None => {
                external += 1;
                None
            }
        }
    }).collect();

    Some((deps, suppressed, ExternalImportCount { module.path, count: external }))
})

// merge per-file results into ScanResult
```

### The four resolution outcomes, detailed

1. **Suppressed by `noupling:ignore`** → no edge, `suppressed_count += 1`.

   Triggered only when `allow_inline_suppression = true` (the default for `noupling scan`; the `hook` command and some test paths pass `false`). The match is run by `is_suppressed(source, line_number)` in `src/scanner/mod.rs`. It looks at two places:

   - The import line itself, for an inline comment containing the string `noupling:ignore`.
   - The line *immediately above* the import, only if it's a standalone comment line starting with `//`, `#`, or `--`.

   Inline syntax examples (all valid):

   ```rust
   use crate::experimental; // noupling:ignore — TODO: refactor out
   ```

   ```python
   # noupling:ignore
   import legacy_module
   ```

   ```haskell
   -- noupling:ignore: layer break sanctioned by ADR-0007
   import Data.Internal
   ```

   The match is a plain substring check on `"noupling:ignore"`. It deliberately does not require a specific format beyond the comment shape.

2. **Resolver returns `Some(path)` and target found** → emit `Dependency { from_module_id, to_module_id, line_number }`.

   The resolver result is matched against the discovered module set (`modules.iter().find(|m| m.path == resolved)`). If the resolver returns a path that's not in the module set — which would be a resolver bug, since `known_paths` is exactly the module-set paths — the edge is silently dropped.

3. **Resolver returns `None`** → no edge, `external_count += 1` for this module.

   The module appears in `external_imports` with a `count` of however many unresolved imports it had. Modules with zero externals aren't added to `external_imports` (the producer filters them out before returning).

4. **Resolver returns a self-edge (`from_id == to_id`)** → drop unconditionally, no counter updated.

   Added in [PR #214](https://github.com/pererikbergman/noupling/pull/214) as defence in depth against the substring-match bug described in § Stage 2. After the `ends_with_segment` fix, self-edges shouldn't happen from any current resolver. But a future resolver bug could re-introduce them, and the guard ensures the graph never carries a `from == to` edge regardless. A file importing itself is never a meaningful coupling event.

### Parallelism

The outer loop is `modules.par_iter()` — Rayon-parallel over modules. Each module's source is read, parsed, and resolved on whichever worker thread is available. Output is deterministic: parsing and resolution are pure, and the per-file results are merged into the final `Vec<Dependency>` in `par_iter` order (which respects the discovery order from Stage 1).

This is the only place the producer does parallel work. Discovery is sequential (filesystem walk); persistence is sequential (single SQLite connection). The graph's per-file work is the parallelizable bit, and Rayon takes the wheel for it.

### What synthesis does NOT do

- Does not score, rank, or weight anything.
- Does not classify dependencies by direction (downward / sibling / upward / circular). That's `analyzer::DependencyDirection`.
- Does not aggregate or summarise. Every `Dependency` produced is one specific import statement at one specific line.
- Does not deduplicate. If a file imports the same target twice (different lines), you get two `Dependency` rows with different `line_number` values. The SQLite schema treats `(from, to, line_number)` as the primary key, so both rows persist.

---

## What's NOT in the graph

Things readers might assume are nodes or edges but aren't.

- **Layers.** `Settings.layers` is consulted by the analyzer, not the scanner. Layer assignment is a post-hoc partitioning of the same graph nodes. See `src/analyzer/layers.rs`.
- **Metrics.** Hotspots, cohesion, abstractness, instability, distance, gravity wells, red flags, blast radius, independence — all derived from the graph by `src/analyzer/*`. The producer doesn't compute any of them.
- **Snapshot history relationships.** `snapshot_id` is a temporal grouping key, not a graph edge. Snapshots aren't linked into a chain in the producer; trend analysis builds those relationships by querying SQLite.
- **Monorepo partitioning.** The scanner produces one unified graph from all discovered files. Monorepo splits and cross-module violations happen post-graph in `src/analyzer/monorepo.rs`.
- **Type counts (abstract vs concrete).** Produced as a side-effect of the parser registry (via `count_type_declarations`), but they feed the abstractness metric in `analyzer::compute_abstractness`, not the graph. They're not nodes, not edges, and not in `ScanResult`. (They're recomputed at audit time by `scanner::recompute_type_counts`, which lives in `src/scanner/mod.rs` purely because that's where the parser registry lives — it's a consumer of parsers, not a graph producer.)
- **Diff mode.** `--diff-base main` is honoured by the *audit* by filtering violations to those involving changed files. The scanner builds the same full graph either way.
- **External-package identity.** When a Python file imports `numpy`, the resolver returns `None` and the import is counted as external. The graph does not contain a node for `numpy`. External imports are *counted*, not *modeled*.

---

## Persistence

The producer's job ends at `ScanResult`. Persistence is handled separately by the `scan` command (`src/commands/scan.rs`), which takes the `ScanResult` and writes it to SQLite via `src/storage/repository.rs`. The producer itself does no database I/O.

### Schema

`.noupling/history.db`, auto-created on first command run. Four tables (`src/storage/db.rs`):

```sql
CREATE TABLE snapshots (
    id TEXT PRIMARY KEY,
    timestamp DATETIME DEFAULT CURRENT_TIMESTAMP,
    root_path TEXT NOT NULL,
    suppressed_count INTEGER NOT NULL DEFAULT 0,
    diff_base TEXT,
    diff_changed_files TEXT
);

CREATE TABLE modules (
    id TEXT PRIMARY KEY,
    snapshot_id TEXT REFERENCES snapshots(id),
    parent_id TEXT REFERENCES modules(id),
    name TEXT NOT NULL,
    path TEXT NOT NULL,
    module_type TEXT CHECK(module_type IN ('FILE', 'DIR')),
    depth INTEGER NOT NULL
);

CREATE TABLE dependencies (
    from_module_id TEXT REFERENCES modules(id),
    to_module_id TEXT REFERENCES modules(id),
    line_number INTEGER,
    PRIMARY KEY (from_module_id, to_module_id, line_number)
);

CREATE TABLE snapshot_external_deps (
    snapshot_id TEXT REFERENCES snapshots(id),
    module_path TEXT NOT NULL,
    count INTEGER NOT NULL,
    PRIMARY KEY (snapshot_id, module_path)
);
```

### What this means for the graph

- One scan produces one snapshot. Each rerun of `noupling scan` creates a new `snapshots` row with a fresh `id`, a new set of `modules` rows (fresh UUIDs even for unchanged files), and a new set of `dependencies` rows.
- The `path` column on `modules` is the stable identifier across snapshots; `id` is stable only within a snapshot. Analysis that wants "is this the same file as last time?" matches on `path`, not `id`.
- The `dependencies` primary key includes `line_number`, so a file importing the same target on two different lines persists as two rows.
- `snapshot_external_deps` holds the side-channel `external_imports` counter. It's not a graph table; it's a per-snapshot summary.

### Type counts deliberately don't persist

Type counts (used by the abstractness metric) are not in the schema. They're recomputed at audit time from the current source files by `scanner::recompute_type_counts`. This is an intentional in-memory-only choice — see the abstractness arc PRs (#69 / #215) for the reasoning. It does not affect the graph; the graph is fully persisted.

---

## Adding a language

If you wanted to add Lua, here's the complete list of changes:

### 1. Create `src/scanner/parsers/lua.rs`

```rust
use tree_sitter::Parser;

use super::{ends_with_segment, ImportEntry, LanguageParser};

pub struct LuaParser;

impl LanguageParser for LuaParser {
    fn parse(&self, source: &str) -> Vec<ImportEntry> {
        // Walk a tree-sitter-lua AST, look for `require(...)` calls,
        // emit ImportEntry { path: <string-arg>, line_number: <1-based> }
        // for each.
        todo!()
    }

    fn resolve(
        &self,
        import_path: &str,
        _source_file: &str,
        known_paths: &[String],
    ) -> Option<String> {
        // Convert `require("foo.bar")` → candidate `foo/bar.lua`,
        // look it up in known_paths using ends_with_segment.
        let candidate = format!("{}.lua", import_path.replace('.', "/"));
        known_paths
            .iter()
            .find(|p| ends_with_segment(p, &candidate))
            .cloned()
    }

    // Skip count_type_declarations unless Lua participates in the
    // abstractness metric — its default returns zeros, which is correct
    // for languages without trait/interface/abstract-class declarations.
}
```

### 2. Add one line to `src/scanner/parsers/mod.rs::registry()`

```rust
("lua", Box::new(lua::LuaParser)),
```

And the corresponding `pub mod lua;` near the top of `parsers/mod.rs`.

### 3. Add tests in `parsers/lua.rs`

At minimum:

- **Parser**: empty source produces no imports; a `require("foo")` produces one `ImportEntry` with the right path and line; multiple requires produce them in source order.
- **Resolver**: an import that matches a known path resolves; an import that would substring-match but isn't a segment match (e.g. `require("re")` against `structure.lua`) does NOT resolve; an import to an unknown name returns `None`.

### 4. Ensure `lua` is in `source_extensions`

For projects that want Lua scanned, they need `"lua"` in `Settings.source_extensions`. The default `Settings::default()` covers the existing 16 languages; adding a new one means either updating the defaults in `src/settings.rs` (project-wide) or having each user opt in via their `.noupling/settings.json`.

That's the entire surface. **No other files need to change.** No analyzer change, no reporter change, no schema change. The trait abstraction was specifically designed for this.

---

## References

### Read in this order

1. `src/core/mod.rs` — the two types the whole graph is made of
2. `src/scanner/discovery.rs` — how files become Modules
3. `src/scanner/parsers/mod.rs` — the trait and helpers
4. `src/scanner/parsers/rust.rs` (or any one adapter) — concrete example of `parse` + `resolve`
5. `src/scanner/mod.rs` — `scan_project`, `is_suppressed`, the edge-decision logic

### Persistence (graph storage, not graph construction)

- `src/storage/db.rs` — schema + auto-migration
- `src/storage/repository.rs` — CRUD wrappers
- `src/commands/scan.rs` — the command that calls `scan_project` and persists the result

### Load-bearing history

- [#212 / PR #214](https://github.com/pererikbergman/noupling/pull/214) — `ends_with_segment` and the substring-match resolver bug. Required reading for anyone writing a new resolver.

### Out of scope (consumer of the graph, not part of it)

- `src/analyzer/` — every metric, every violation type, every score. All operate on `(modules, dependencies)`.
- `src/reporter/` — every output format. All operate on `AuditResult`, which is derived from the graph.
