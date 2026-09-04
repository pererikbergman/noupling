// STABILITY VIOLATION: stable (imported by main, one outgoing) depends on volatile (many outgoing, one incoming).
use crate::volatile::v;
pub fn run() { v::v(); }
