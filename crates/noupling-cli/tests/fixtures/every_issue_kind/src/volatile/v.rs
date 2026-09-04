// High instability: two outgoing, one incoming.
use crate::legacy::compat;
use crate::domain::order;
pub fn v() { compat::shim(); order::Order; }
