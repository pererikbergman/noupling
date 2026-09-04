// Entry point. Imports here are parent → child (downward) and never count as coupling.
use crate::ui::screen;
use crate::concrete::types;
use crate::stable::core;
use crate::bag::a;
fn main() { screen::draw(); types::T; core::run(); a::a(); }
