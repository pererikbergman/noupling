// Internal to infra.
use crate::infra::db;
pub fn warm() { db::persist(); }
