// LAYER VIOLATION: infra (bottom) imports ui (top).
use crate::ui::screen;
pub fn persist() { screen::draw(); }
