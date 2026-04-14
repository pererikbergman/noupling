//! SQLite persistence layer for snapshots, modules, and dependencies.

mod db;
pub mod repository;

pub use db::Database;
