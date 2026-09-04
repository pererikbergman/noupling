// FUSED SIBLING: every left file imports both right files.
use crate::fused::right::r1;
use crate::fused::right::r2;
pub fn l2() { r1::r1(); r2::r2(); }
