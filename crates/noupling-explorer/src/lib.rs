//! Stub crate for the Explorer report format.
//!
//! Task 1 of #228 (#229) lands this empty shell so the workspace boundary
//! exists. Task 2 onward populates it with the data contract, template
//! assembly, and `--format explorer` wiring.
//!
//! Dependency rule (enforced via `.noupling/settings.json`):
//! `noupling-explorer` may depend only on `noupling-core`. It must not
//! depend on `noupling` (the cli crate) or on any reporter sibling.
