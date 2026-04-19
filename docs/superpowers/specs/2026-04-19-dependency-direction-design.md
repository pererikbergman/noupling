# Phase 1: Dependency Direction Classification

**Issue:** #168
**Part of:** #167 — Software Dependency Risk Framework (RRI/TRI)
**Date:** 2026-04-19

## Purpose

Add a `DependencyDirection` enum that classifies every violation by its architectural direction. This is the foundation for Phase 2 (severity weights + RRI calculation) where each direction gets a distinct risk weight.

## Direction Enum

```rust
pub enum DependencyDirection {
    Downward,  // parent dir imports from child dir (weight 2 in Phase 2)
    Sibling,   // same-level dirs import each other (weight 4 in Phase 2)
    Upward,    // child dir imports from parent dir (weight 6 in Phase 2)
    Circular,  // cycle between dirs (weight 10 in Phase 2)
}
```

Derives: `Debug`, `Clone`, `Serialize`, `Deserialize`, `PartialEq`, `Eq`.
Serde renames variants to lowercase (`"downward"`, `"sibling"`, `"upward"`, `"circular"`).

## Classification Rules

Direction is a **violation-level** classification, not per-import:

| Condition | Direction |
|-----------|-----------|
| `is_circular = true` | `Circular` |
| All other violations (sibling-pair coupling) | `Sibling` |

`Downward` and `Upward` are defined but unused until Phase 2 adds detection for those dependency types.

## Changes

### `src/core/mod.rs`
- Add `DependencyDirection` enum with serde support
- Add serde roundtrip test for the enum

### `src/analyzer/mod.rs`
- Add `pub direction: DependencyDirection` field to `CouplingViolation`
- Set `Circular` at all circular violation construction sites (line ~863, ~602)
- Set `Sibling` at all coupling violation construction sites (line ~903, ~947)
- Update `make_coupling` test helper to include `direction: Sibling`
- Add test: verify circular violations get `Circular`, sibling violations get `Sibling`

### `src/baseline.rs`
- Update `make_coupling` test helper to include `direction` field

## Not In Scope

- New violation detection (upward/downward) — Phase 2
- Weight/RRI calculation — Phase 2
- Report display changes — Phase 6
- External/transitive dependencies — Phase 7

## Tests

1. `DependencyDirection` serde roundtrip (all 4 variants)
2. Sibling coupling violations classified as `Sibling`
3. Circular dependency violations classified as `Circular`
4. All 185 existing tests pass with new field
