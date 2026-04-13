---
description: Restructure workspace and establish vertical slice architecture with all core dependencies
type: Spec
---

# Epic: Project Foundation

### 1. Objective & Value Proposition

* **Goal:** Restructure the Cargo workspace from the generic scaffold into the noupling CLI vertical slice architecture, adding all required dependencies.
* **User Value:** Enables all subsequent development by providing a compilable, correctly structured project with proper domain boundaries.

### 2. User Stories

* As a developer, I want the workspace restructured into vertical slices (scanner, storage, analyzer, reporter) so that each domain is physically isolated.
* As a developer, I want all core dependencies (clap, tree-sitter, rusqlite, rayon, fxhash) declared in the workspace so that slices can reference them consistently.
* As a developer, I want a single binary crate (`noupling`) with a `clap` CLI skeleton so that I can run subcommands immediately.
* As a developer, I want shared domain types (Node, Dependency) defined in a core module so that slices have a common vocabulary.
* As a developer, I want error handling utilities in place so that all slices use a consistent error pattern.

### 3. Functional Requirements

* Remove placeholder crates (api_app, core_logic, shared_types).
* Create the noupling binary crate with clap CLI skeleton (scan, audit, report subcommands as stubs).
* Create vertical slice modules: scanner, storage, analyzer, reporter.
* Define core domain types: Node (with FILE/DIR type), Dependency, Snapshot.
* Set up tracing/logging infrastructure.

### 4. Acceptance Criteria

- [ ] `cargo build` compiles successfully.
- [ ] `cargo run -- --help` shows scan, audit, report subcommands.
- [ ] Each slice module exists with a `mod.rs` stub.
- [ ] Core domain types compile with serde Serialize/Deserialize.
- [ ] `cargo clippy` passes with no warnings.
- [ ] `cargo fmt --check` passes.
