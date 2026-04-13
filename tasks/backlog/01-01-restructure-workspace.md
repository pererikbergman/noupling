---
description: Restructure Cargo workspace from generic scaffold to noupling vertical slice architecture
type: Task
story: 01-01
---

# Task: Restructure Workspace into Vertical Slice Architecture

### Context

The current workspace has placeholder crates (api_app, core_logic, shared_types) from the initial scaffold. The noupling spec requires a single binary crate with vertical slice modules.

### Objective

Replace the placeholder crates with the noupling binary crate containing the vertical slice module structure.

### Acceptance Criteria

- [ ] Placeholder crates (api_app, core_logic, shared_types) are removed.
- [ ] A single `noupling` binary crate exists at `crates/noupling/`.
- [ ] Vertical slice modules exist: `slices/scanner/mod.rs`, `slices/storage/mod.rs`, `slices/analyzer/mod.rs`, `slices/reporter/mod.rs`.
- [ ] Core module exists: `core/mod.rs`.
- [ ] Utils module exists: `utils/mod.rs`.
- [ ] `cargo build` compiles successfully.
- [ ] `cargo clippy` passes.

### Technical Details

**Files to remove:**
- `crates/api_app/` (entire directory)
- `crates/core_logic/` (entire directory)
- `crates/shared_types/` (entire directory)

**Files to create:**
- `crates/noupling/Cargo.toml`
- `crates/noupling/src/main.rs`
- `crates/noupling/src/slices/mod.rs`
- `crates/noupling/src/slices/scanner/mod.rs`
- `crates/noupling/src/slices/storage/mod.rs`
- `crates/noupling/src/slices/analyzer/mod.rs`
- `crates/noupling/src/slices/reporter/mod.rs`
- `crates/noupling/src/core/mod.rs`
- `crates/noupling/src/utils/mod.rs`

**Root Cargo.toml:** Update workspace members (already uses `crates/*` glob, so no change needed).

### Implementation Steps

- [ ] 1. Remove placeholder crate directories.
- [ ] 2. Create noupling crate directory structure.
- [ ] 3. Write `crates/noupling/Cargo.toml` with workspace references.
- [ ] 4. Write stub `main.rs` with module declarations.
- [ ] 5. Write stub `mod.rs` files for each slice and core/utils.
- [ ] 6. Verify `cargo build` and `cargo clippy`.
