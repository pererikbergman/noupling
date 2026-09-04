// FUSED SIBLING / GRAVITY WELL: l1 carries both of left's relationships
// (left → right, weight 6, and left → mid, weight 1) so its aggregate RRI
// is far above the median and it counts ≥ 2 relationships.
use crate::fused::right::r1;
use crate::fused::right::r2;
use crate::fused::mid::m;
pub fn l1() { r1::r1(); r2::r2(); m::m(); }
