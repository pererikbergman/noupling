// FUSED SIBLING: every left file imports both right files.
use crate::fused::right::r1;
use crate::fused::right::r2;
pub fn l3() { r1::r1(); r2::r2(); }
