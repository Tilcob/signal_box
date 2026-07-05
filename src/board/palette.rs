//! The Pult color palette. The "lit" colours are authored as HDR (linear
//! components > 1.0) to feed the bloom pass. With bloom off (the default —
//! `STELLWERK_BLOOM` unset, see [`crate::camera`]) the LDR framebuffer would
//! hard-clip them to white, so they are Reinhard-tonemapped into [0,1) first,
//! keeping their hue.

use bevy::prelude::*;
use std::sync::LazyLock;
use stellwerk_sim::units::BlockId;

/// Mirrors the camera's `STELLWERK_BLOOM` check (same env contract): when the
/// HDR/bloom path is active, lit colours pass through raw; otherwise they are
/// tonemapped. Resolved once per process — a test that varies STELLWERK_BLOOM
/// between cases in the same process would see a stale value (none do).
static BLOOM: LazyLock<bool> = LazyLock::new(|| std::env::var_os("STELLWERK_BLOOM").is_some());

/// A lit (overbright) colour. Raw HDR when bloom consumes it; otherwise scaled
/// into gamut by a single Reinhard factor on the brightest channel, so all
/// three channels keep their ratio — the hue is preserved exactly, only the
/// overbright glow is gone (it clipped to white without this).
fn lit(r: f32, g: f32, b: f32) -> Color {
    if *BLOOM {
        Color::LinearRgba(LinearRgba::rgb(r, g, b))
    } else {
        let peak = r.max(g).max(b);
        let f = 1.0 / (1.0 + peak); // Reinhard on the peak, applied uniformly
        Color::LinearRgba(LinearRgba::rgb(r * f, g * f, b * f))
    }
}

/// Buildable-cell tile. Kept well above the near-black desk
/// (`ClearColor` ≈ 0.015) so the build area reads at a glance — new players
/// were missing where they could draw. Still below fixed/player track
/// (0.16/0.30) and the cell-index text (0.21), so tiles never outshout content.
pub fn col_grid() -> Color {
    Color::srgb(0.065, 0.075, 0.095)
}
/// Sandbox block (non-buildable cell): a solid slate tile, clearly heavier
/// than the faint grid square and the near-black desk so it reads as a wall.
/// Tune freely — purely visual, never lit/HDR.
pub fn col_blocked() -> Color {
    Color::srgb(0.11, 0.10, 0.13)
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
    lit(1.6, 1.2, 0.25)
}
pub fn col_switch_inactive() -> Color {
    Color::srgb(0.10, 0.10, 0.08)
}
pub fn col_signal_green() -> Color {
    lit(0.25, 2.2, 0.5)
}
pub fn col_signal_red() -> Color {
    lit(2.6, 0.25, 0.2)
}
pub fn col_occupied() -> Color {
    lit(1.5, 0.30, 0.22)
}
pub fn col_reserved() -> Color {
    lit(1.3, 0.95, 0.20)
}
pub fn col_train() -> Color {
    lit(2.4, 1.9, 1.1)
}
pub fn col_head() -> Color {
    lit(4.0, 3.2, 1.8)
}
/// Freight train body: a cool teal, clearly not the warm-white passenger train
/// ([`col_train`]) — the class read is by colour, no text (anti-text principle).
pub fn col_freight() -> Color {
    lit(0.9, 1.7, 2.2)
}
/// Freight train head lamp — the brighter tip of [`col_freight`].
pub fn col_freight_head() -> Color {
    lit(1.6, 3.0, 3.8)
}
/// Shrinking dwell ring drawn at a freight train halted at its platform.
pub fn col_dwell() -> Color {
    lit(2.6, 2.0, 0.4)
}
pub fn col_label() -> Color {
    Color::srgb(0.55, 0.58, 0.65)
}
/// Source station (trains enter here): a cool entry colour, kept clear of
/// signal-green and switch-amber so a station never reads as track gear.
/// Purely visual, tune freely.
pub fn col_source() -> Color {
    Color::srgb(0.32, 0.58, 0.74)
}
/// Sink station (trains terminate here): a warm arrival colour, paired against
/// [`col_source`]. Purely visual.
pub fn col_sink() -> Color {
    Color::srgb(0.80, 0.47, 0.30)
}
/// Freight platform (a drive-through unload stop): a distinct dock green, kept
/// apart from source-blue and sink-orange. Purely visual.
pub fn col_platform() -> Color {
    Color::srgb(0.34, 0.62, 0.45)
}
/// Bright lip along the platform edge facing the track (the "Bahnsteigkante").
pub fn col_platform_edge() -> Color {
    Color::srgb(0.62, 0.90, 0.72)
}
/// The plain block behind the platform — a shade darker so it reads as a
/// separate structure, not part of the edge.
pub fn col_platform_back() -> Color {
    Color::srgb(0.27, 0.50, 0.37)
}
/// Cell index in buildable tiles: readable on the dark desk, but clearly
/// quieter than tracks and labels.
pub fn col_cell_index() -> Color {
    Color::srgb(0.21, 0.24, 0.31)
}
