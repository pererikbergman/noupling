//! noupling-core — pure analysis library.
//!
//! Public surface consumed by `noupling` (the cli crate) and, later,
//! `noupling-explorer`. Contains the scanner, analyzer, storage, layer/rule
//! engine, settings loader, baseline, and diff utilities. Has zero
//! awareness of any reporter or CLI concern: removing every reporter
//! must leave this crate compiling.

pub mod analyzer;
pub mod baseline;
pub mod core;
pub mod diff;
pub mod scanner;
pub mod settings;
pub mod storage;
pub mod utils;
