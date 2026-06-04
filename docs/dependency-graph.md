# The noupling Dependency Graph

A short, pure-model write-up. No scanner. No Rust. No file paths. Just the graph.

## The shape

A noupling dependency graph is a **directed multigraph**:

- **Vertices** are source files. One file = one vertex.
- **Edges** are imports. The edge `A → B` reads as "file A imports something defined in file B."
- **Direction matters.** `A → B` and `B → A` are different edges — removing B breaks A, removing A doesn't break B. The asymmetry is the whole reason coupling matters.
- **Parallel edges are allowed.** If A imports B on line 3 *and* on line 17, that's two edges between the same pair, distinguished by line number. They aren't deduplicated.
- **No self-loops.** `A → A` is never present. A file importing itself isn't a coupling event; the model enforces this at construction time so every consumer can assume it.
- **Closed under the project boundary.** Only project files are vertices. An import to stdlib (`import os`) or to a third-party package (`import numpy`) does not introduce a vertex — it's counted as an external import in a side channel, but it isn't part of the graph.

Formally:

```
G = (V, E)

V ⊆ { source files in the project }
E ⊆ V × V × ℤ⁺          -- third component is line number
(v, v, k) ∉ E            -- no self-loops, for any v and any k
```

## A worked example

Picture a small Python project:

```
src/
  main.py     ── imports config, models
  config.py   ── (imports nothing in-project)
  models.py   ── imports util
  util.py     ── (imports nothing in-project)
```

The graph:

```
              ┌────────┐
              │ main   │
              └───┬────┘
              ┌───┴────┐
              ▼        ▼
         ┌────────┐  ┌────────┐
         │ config │  │ models │
         └────────┘  └────┬───┘
                          ▼
                     ┌─────────┐
                     │ util    │
                     └─────────┘
```

V = {main, config, models, util}, |V| = 4
E = {(main, config, _), (main, models, _), (models, util, _)}, |E| = 3

All edges run downward; there are no cycles. This is a DAG.

## Properties of a vertex

For any vertex v:

| Property | Definition | What it tells you |
|---|---|---|
| **out-degree(v)** | count of edges leaving v | How many other files v depends on. High = *unstable*: v has many reasons to change |
| **in-degree(v)** | count of edges entering v | How many other files depend on v. High = *stable*: v's changes ripple widely |
| **reachable-from(v)** | the set of vertices walkable along edges out of v | Everything v transitively depends on |
| **reaches-to(v)** | the set of vertices that can walk along edges to v | Everything that transitively depends on v — the **blast radius** |

In the example above:
- `main`: out-degree 2, in-degree 0, blast radius 0 (nothing depends on main)
- `util`: out-degree 0, in-degree 1, blast radius 2 (changing util ripples to models, which ripples to main)
- `config`: out-degree 0, in-degree 1, blast radius 1 (only main)

## Properties of the graph as a whole

- **Connected components** — the disjoint islands. A healthy modular system has several; a tangled system collapses into one giant blob.
- **Strongly connected components (SCCs)** — sets of vertices where every member can reach every other member via directed paths. An SCC of size > 1 is a **cycle**. Cycles in a code-dependency graph are nearly always bad: they create files that must be built, tested, and reasoned about as a unit.
- **Acyclicity** — a graph with zero SCCs of size > 1 is a DAG. DAG-shaped codebases admit topological order: you can compile from the leaves up, you can refactor a leaf without touching its callers, you can test the whole project in dependency order. Cycles break all of that.

If you added one edge `util → main` to the example, you'd create an SCC of size 4 (everything reaches everything). The DAG would collapse to a single tangled cycle.

## Every metric is a graph operation

Underneath every named metric in noupling is a query on (V, E):

| Metric / finding | Underlying query |
|---|---|
| Hotspot | `for v in V: in-degree(v)` — pick the largest |
| Unstable module | `for v in V: out-degree(v) / (in-degree(v) + out-degree(v))` |
| Blast radius | `for v in V: \|reaches-to(v)\|` |
| Cycle | `tarjan-scc(G)` — pick SCCs of size > 1 |
| Coupling between two areas S, T ⊆ V | `\|{ (a, b, _) ∈ E : a ∈ S, b ∈ T }\|` |
| Cohesion of an area S ⊆ V | `\|{ (a, b, _) ∈ E : a ∈ S, b ∈ S }\| / \|S\|·(\|S\|−1)` |
| Dependency depth | longest path in the DAG (when one exists) |

Different names. Same graph. Different ways of asking.

## What is not in the graph

The model is deliberately minimal. The graph carries:

- vertices
- directed edges
- line numbers as edge labels (only to disambiguate parallel edges)

It carries **no**:

- **Edge weights.** All edges are equal. Risk weighting (sibling vs. upward vs. circular) is interpretation layered on later, by walking the graph and assigning weights at query time.
- **Vertex types or tags.** A vertex is "a file." It is not "a controller" or "a domain entity" or "part of the auth layer." Layering, ownership, and architectural roles are configuration overlaid on top of the graph.
- **History.** The graph is a snapshot of one moment in time. Trend analysis builds many graphs (one per snapshot) and compares them.
- **External vertices.** Stdlib and third-party imports never become vertices. They're counted in a side channel ("external imports per module") because that count is occasionally useful, but they don't participate in graph operations.
- **Granularity below the file level.** No class nodes, no function nodes. A file-to-file edge exists if file A imports any name from file B; it doesn't matter whether the name is a class, function, constant, or wildcard.

## Why this minimum

Keeping the graph this thin has two payoffs:

1. **Every downstream feature is provably a function of (V, E).** Cohesion, coupling, instability, hotspots, layers, scoring — you can describe each one as a query on the graph without referring to the source code that produced it. If a metric *can't* be expressed as a graph query, that's a smell.
2. **Replacement is cheap.** Swap how you build the graph (different language parsers, AST-based vs. regex-based, a totally different scanner) and nothing downstream changes. The graph is the contract.

---

That's the model. Vertices, directed edges, parallel edges allowed, no self-loops, closed under the project boundary. Everything else noupling does is a query on this object.

---

# Analysis, Step 1: Lift the graph onto the directory tree

The graph above has files as vertices. Analysis cares about something different: how **groups of files** depend on each other. The first step of analysis is to overlay a tree on the graph and aggregate dependencies up that tree.

## A note on terminology: ply vs depth

Two distinct "depth" concepts show up in this analysis. To keep them straight, this document uses:

- **ply** — *tree-depth*. How far a directory sits from the project root in the directory hierarchy. `com.myapp.app.utils` is at **ply 4**. The directory tree is processed one ply at a time, bottom-up. Borrowed from chess / tree-search literature where each level down is one ply.
- **depth** — *graph-depth*. The length of the longest dependency *chain* through the file graph (`file → file → file → …`). This is what the existing `max_depth` and `critical_path` outputs measure. A different number, computed by walking edges, not the directory tree.

Plies are about the tree overlay; depth is about paths through the dependency graph itself. Mixing the two has been a recurring confusion — this doc keeps them separated.

(Note: the code currently uses `Module.depth` and `dir_depth()` to mean what this doc calls *ply*. That's a known collision; this doc opts for the cleaner terminology and code may eventually catch up.)

## The tree

The directory hierarchy is the tree. Every file vertex has exactly one parent directory; every directory has a parent directory; and so on up to the project root.

```
                       ┌─────────────┐
                       │  A  (dir)   │
                       └──┬──────┬───┘
                          │      │
                  ┌───────┘      └────────┐
                  ▼                       ▼
            ┌─────────┐              ┌─────────┐
            │ B (dir) │              │ C (dir) │
            └────┬────┘              └─────────┘
                 │
                 ▼
            ┌─────────┐
            │ D (dir) │
            └─────────┘
```

A has children B and C. B has child D. D contains files. C contains files. B may also contain its own direct files alongside D.

Note: this is a **tree of directories**, sitting on top of the **graph of files**. The two structures coexist. Files are still the vertices of the dependency graph; directories are the aggregation buckets.

## The accumulated-dependency rule

For each directory node `X`, define:

```
D_acc(X) = { every dependency edge (s, t, _) ∈ E
                such that
                    s is some file in X's subtree
                AND
                    t is some file NOT in X's subtree }
```

In English: D_acc(X) is the set of edges that **cross out** of X's subtree.

Two parts to that rule:

1. The source must be **somewhere under X** — directly in X, or in a child of X, or in a grand-child, recursively.
2. The target must be **outside X's subtree** — not in X, not in any descendant of X.

Edges where both endpoints are inside X's subtree don't count. They're *internal* to X and invisible at the X level.

## Aggregation by ply

Compute D_acc bottom-up by ply. Process the highest-ply (deepest) directories first; each parent inherits its children's D_acc, *filtered to drop anything that became internal at the parent's level*.

```
D_acc(D)  = { edges out of D's files that leave D's subtree }
D_acc(B)  = { edges out of B's direct files that leave B }
          ∪ { edges in D_acc(D) whose target is still outside B }
D_acc(C)  = { edges out of C's files that leave C }
D_acc(A)  = { edges out of A's direct files that leave A }
          ∪ { edges in D_acc(B) whose target is still outside A }
          ∪ { edges in D_acc(C) whose target is still outside A }
```

Three things are happening on each upward step:

1. **Inherit** every child's D_acc.
2. **Re-filter**: anything that was "external to the child" might be "internal to the parent" (the parent's subtree is larger). Drop those.
3. **Add** the parent's *own direct files'* outgoing edges that leave the parent's subtree.

## A worked example

Use the structure from the picture: A has children B and C; B has child D. Suppose the file-level edges are:

| edge | meaning |
|---|---|
| `D.f1 → C.f1` | D's file depends on a file in C |
| `D.f1 → B.f1` | D's file depends on a file directly in B |
| `B.f1 → C.f1` | B's direct file depends on a file in C |
| `B.f1 → outside-A` | B's direct file depends on a file *outside the A subtree entirely* |

Walking the levels:

```
D_acc(D)  = { D→C , D→B , D→outside }
            ↑ everything D's files reach that's outside D itself

D_acc(B)  = { B→C , B→outside }            (B's direct edges that leave B)
          ∪ { D→C , D→outside }            (inherited from D, still external to B)
                                            -- D→B drops here because B's file
                                               is now inside B's own subtree

         = { B→C , B→outside , D→C , D→outside }

D_acc(C)  = { }   (C had no outgoing edges in this example)

D_acc(A)  = { }                            (no direct edges on A's own files)
          ∪ { B→outside , D→outside }       (from D_acc(B), still external to A)
                                            -- B→C and D→C drop here because
                                               C is inside A's subtree
          ∪ { }                            (nothing inherited from C)

         = { B→outside , D→outside }
```

Walk-through of what your intuition predicts vs. what actually happens:

> "If I ask for B's dependencies, they include D's dependencies."

Confirmed — D_acc(B) inherits from D_acc(D). The set `{ D→C , D→outside }` shows up at B's level.

The subtlety: `D→B` is in D_acc(D) but **disappears** at B's level. Once you're standing at B, D depending on B's own files is no longer a cross-boundary edge.

Similarly going from B up to A: edges to C (which is inside A's subtree) drop out, but edges that leave A entirely persist.

## Why aggregate this way

Two reasons.

**1. Coupling is a boundary phenomenon.** "Module X depends on module Y" is only meaningful when there's a boundary between X and Y. Aggregating up the tree and filtering at each level is exactly how you ask "at this granularity, what crosses?" — letting you spot coupling between siblings (`B` vs. `C`) without being misled by perfectly fine internal coupling within either.

**2. Bottom-up is efficient.** Each directory is processed once, after all its children. No directory's D_acc has to be recomputed from scratch — it just merges-and-filters its children's results. The whole pipeline is O((files + dirs) × edges) instead of the naïve O(dirs² × edges).

## How this feeds the rest of analysis

D_acc is the input to the next step of analysis: **detect sibling coupling and circular dependencies between sibling directories.** At every level of the tree, walk the siblings and ask:

- For each pair (A, B) at this level: does D_acc(A) contain any target inside B? If yes → A is *sibling-coupled* to B.
- Among the siblings at this level: build a small directed graph using D_acc and run Tarjan's SCC. SCCs of size > 1 are *circular dependencies* between sibling directories.

That's the bridge from "graph of files" to "graph of directories" — and that directory-level graph is what every downstream metric (cohesion, instability, layers, risk weights, scoring) operates on.

---

# Analysis, Step 2: Cohesion

Once the directory tree is overlaid and D_acc has flowed upward, the next question is **how internally knit is each directory?** That's cohesion. It complements coupling (which asks how directories depend on *each other*) by asking how the inside of one directory hangs together.

## Two kinds of directory nodes

Every directory in the tree is one of:

| Kind | Definition | Cohesion | Coupling |
|---|---|---|---|
| **Container** | A directory with zero direct files — only subdirectories live below it. Examples: `src/features/` holding `auth/ billing/ checkout/`; `src/domain/` grouping bounded contexts. | **undefined** | defined |
| **Package** | A directory with at least one direct file. Examples: `src/scanner/` containing `mod.rs` and `discovery.rs`; any leaf-level folder of code. | defined | defined |

This split is automatic — there's no human-tagged "this is a grouping folder." The tree shape alone decides it. A `features/` that contains only feature folders is structurally a container; the moment someone drops a `shared.rs` into it, it becomes a package and starts having cohesion.

Coupling works for both kinds because it's a *between-nodes* measure: count edges crossing from somewhere in one subtree to somewhere in another. The contents of either subtree don't matter to the count. Cohesion is a *within-a-node* measure, and containers have no within to measure.

## Why containers don't get cohesion

A container's purpose is to group, not to implement. Asking "how cohesive is `features/`?" is the wrong question — the right question for `features/` is "how coupled are the features inside it to each other?" That's coupling, not cohesion. Reporting `cohesion(features/) = 0.00` (as a naïve implementation would) is actively misleading: it reads as "low cohesion, bad" when the reality is "this isn't the kind of node that has cohesion."

The undefined value is therefore not "missing data" — it's a **classification**. Seeing `cohesion: null` on a node tells you, unambiguously, that the node is a container. Combined with the package-vs-container dichotomy above, this means cohesion output doubles as automatic detection of grouping folders.

## Cohesion of a package

For a package `X`, treat each direct tree-child as a single **logical node**, regardless of whether it's a file or a subdirectory:

```
children(X)   = direct files of X  ∪  direct subdirectories of X
                (each counted once; subdirectories are opaque at this level)

n             = |children(X)|
pairs         = n × (n − 1)              -- directed ordered pairs of distinct children

internal(X)   = count of file-level edges (s, t, _) ∈ E such that
                   s belongs to some child L_a ∈ children(X)
                   t belongs to some child L_b ∈ children(X)
                   L_a ≠ L_b
                -- equivalently: edges within X's subtree that cross from
                   one direct child's "world" into another's

cohesion(X)   = internal(X) / pairs
```

A subdirectory is *opaque* in this calculation: an edge from `scanner/foo.rs → scanner/bar.rs` counts as **inside the `scanner` logical node** when measuring `src/`'s cohesion — that's `scanner`'s internal business, not `src/`'s. Only edges that cross *between* `src/`'s direct children show up.

## A worked example

Consider `src/` containing the direct file `main.rs` plus the subdirectories `scanner/` and `storage/`:

```
src/
├── main.rs        (direct file)
├── scanner/       (subdirectory containing many files)
└── storage/       (subdirectory containing many files)
```

children(src) = `{ main.rs, scanner, storage }` → n = 3, pairs = 6

Now consider these file-level edges in E:

| edge | belongs-to | belongs-to | counted? |
|---|---|---|---|
| `main.rs → scanner/foo.rs` | main.rs | scanner | ✅ across children |
| `scanner/x.rs → scanner/y.rs` | scanner | scanner | ❌ same logical node |
| `scanner/x.rs → storage/y.rs` | scanner | storage | ✅ across children |
| `scanner/x.rs → storage/z.rs` | scanner | storage | ✅ across children (parallel edges count) |
| `main.rs → storage/q.rs` | main.rs | storage | ✅ across children |

`internal(src) = 4`, `cohesion(src) = 4 / 6 ≈ 0.67`.

`scanner/x.rs → scanner/y.rs` doesn't contribute to `src/`'s cohesion — it's inside one logical node. It *would* contribute to `scanner/`'s cohesion if `scanner/` is itself a package (i.e., has direct files alongside its subdirs, or is a leaf package of files).

## What "undefined" looks like in practice

In the model's pseudo-API:

```rust
struct DirectoryCohesion {
    dir: String,
    kind: DirectoryKind,        // Container | Package
    cohesion: Option<f64>,      // None when kind == Container
    n_children: usize,
    internal_edges: usize,
}
```

`Option<f64>` is the principled representation: `None` is the container case, `Some(value)` is the package case. A code-level shortcut using `-1.0` as a sentinel works too, but the model treats "undefined" as a meaningful value, not a placeholder for "missing."

In rendered output:
- JSON: `"cohesion": null`
- Text / HTML: `—`
- The container kind itself is part of the output, so consumers can distinguish "no value because container" from any other absence.

## Range and parallel-edge interaction

Cohesion ranges `[0, ∞)` in principle, not `[0, 1]`. Parallel edges (multiple imports between the same file pair, distinguished by line number) all count toward `internal`. A package whose two direct children import each other 30 times will exhibit `cohesion > 1.0` if `pairs` is small — that's a signal of heavy cross-coupling between subareas, not a bug. The denominator captures "how many child-pairs could be coupled"; the numerator captures "how many edges actually cross." Both are useful; their ratio collapses them into one number, with the caveat that the ratio can exceed 1.0 in dense projects.

Readers comparing across projects should bear this in mind: cohesion is comparable *within* a project (same parallel-edge density assumptions) but less so across projects of different sizes and import styles.

## Recap

- Containers (no direct files) are classified automatically by tree shape; cohesion is undefined for them. Containers still participate in coupling.
- Packages (≥ 1 direct file) get cohesion computed by treating each direct tree-child as a single logical node, regardless of whether the child is a file or a subdirectory.
- Subdirectories are opaque inside their parent's cohesion calculation — their internals contribute only to their own cohesion at their own ply.
- The output explicitly carries the kind, so `cohesion: null` always means "container," never "computation failed."

*(This document describes the intended model. The current `analyzer/cohesion.rs` implementation is narrower: it considers only file ↔ file edges within the same directly-parented directory, treating subdirectories as invisible rather than as opaque logical nodes. The model above is the cleaner one this doc commits to; code may follow.)*

