// COUPLING VIOLATION: sibling x → y, weight 1.
use crate::loose::y::y1;
pub fn x() { y1::y(); }
