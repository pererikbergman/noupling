// RULE VIOLATION: settings forbid plugins → legacy.
use crate::legacy::compat;
pub fn export() { compat::shim(); }
