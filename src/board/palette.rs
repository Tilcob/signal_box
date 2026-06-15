//! The Pult color palette. HDR values > 1.0 feed the bloom pass.

use bevy::prelude::*;
use stellwerk_sim::units::BlockId;

pub fn col_grid() -> Color {
    Color::srgb(0.030, 0.035, 0.045)
}
pub fn col_fixed() -> Color {
    Color::srgb(0.16, 0.19, 0.24)
}
pub fn col_player() -> Color {
    Color::srgb(0.30, 0.34, 0.42)
}
/// Idle colour of a block, distinct per block so a player can see at a glance
/// where their signals cut the net. Hue rotates by the golden angle for
/// maximal separation between adjacent ids; kept muted (low saturation/
/// lightness) so live states (occupied/reserved/train) still read louder.
pub fn col_block(block: BlockId) -> Color {
    let hue = (block.0 as f32 * 137.508).rem_euclid(360.0);
    Color::hsl(hue, 0.45, 0.55)
}
pub fn col_switch_active() -> Color {
    Color::LinearRgba(LinearRgba::rgb(1.6, 1.2, 0.25))
}
pub fn col_switch_inactive() -> Color {
    Color::srgb(0.10, 0.10, 0.08)
}
pub fn col_signal_green() -> Color {
    Color::LinearRgba(LinearRgba::rgb(0.25, 2.2, 0.5))
}
pub fn col_signal_red() -> Color {
    Color::LinearRgba(LinearRgba::rgb(2.6, 0.25, 0.2))
}
pub fn col_occupied() -> Color {
    Color::LinearRgba(LinearRgba::rgb(1.5, 0.30, 0.22))
}
pub fn col_reserved() -> Color {
    Color::LinearRgba(LinearRgba::rgb(1.3, 0.95, 0.20))
}
pub fn col_train() -> Color {
    Color::LinearRgba(LinearRgba::rgb(2.4, 1.9, 1.1))
}
pub fn col_head() -> Color {
    Color::LinearRgba(LinearRgba::rgb(4.0, 3.2, 1.8))
}
pub fn col_label() -> Color {
    Color::srgb(0.55, 0.58, 0.65)
}
/// Cell index in buildable tiles: readable on the dark desk, but clearly
/// quieter than tracks and labels.
pub fn col_cell_index() -> Color {
    Color::srgb(0.21, 0.24, 0.31)
}
