//! Placement variants (R-cycled presets) and placement gates.
//!
//! Hard placement rules (occupied/off-board) are rejected at the tool
//! instead of being placed and flagged: stacking a switch on a switch is
//! never a puzzle state worth inspecting. Cross-cell problems (junction
//! without switch, reachability) stay non-modal — they glow as diagnostics.

use stellwerk_sim::grid::{Cell, Dir8, pair_len};
use stellwerk_sim::layout::{Layout, TrackPiece};
use stellwerk_sim::level::Level;

/// All 16 legal connector pairs, R-cycled for click placement.
pub(super) fn piece_variants() -> Vec<(Dir8, Dir8)> {
    let mut out = Vec::new();
    for a in Dir8::ALL {
        for b in Dir8::ALL {
            if a.index() < b.index() && pair_len(a, b).is_some() {
                out.push((a, b));
            }
        }
    }
    out
}

/// 8 switch presets: cardinal stem, branches = straight-through + 45° turn.
pub(super) fn switch_variants() -> Vec<(Dir8, [Dir8; 2])> {
    let rot = |d: Dir8, k: usize| Dir8::ALL[(d.index() as usize + k) % 8];
    let mut out = Vec::new();
    for stem in [Dir8::W, Dir8::E, Dir8::N, Dir8::S] {
        let straight = stem.opposite();
        out.push((stem, [straight, rot(straight, 1)]));
        out.push((stem, [straight, rot(straight, 7)]));
    }
    out
}

/// Buildable cell, no switch there, and both connectors still free —
/// crossings with disjoint connectors stay legal.
pub(super) fn can_place_piece(level: &Level, merged: &Layout, piece: &TrackPiece) -> bool {
    level.buildable.contains(&piece.cell)
        && !merged.switches.iter().any(|s| s.cell == piece.cell)
        && !merged.pieces.iter().any(|p| {
            p.cell == piece.cell && [p.a, p.b].iter().any(|d| *d == piece.a || *d == piece.b)
        })
}

/// Switch cells are exclusive: buildable and completely empty.
pub(super) fn can_place_switch(level: &Level, merged: &Layout, cell: Cell) -> bool {
    level.buildable.contains(&cell)
        && !merged.pieces.iter().any(|p| p.cell == cell)
        && !merged.switches.iter().any(|s| s.cell == cell)
}

/// Signals need track under their connector and may not stack.
pub(super) fn can_place_signal(level: &Level, merged: &Layout, cell: Cell, at: Dir8) -> bool {
    level.buildable.contains(&cell)
        && merged.has_stub(cell, at)
        && !merged.signals.iter().any(|s| s.cell == cell && s.at == at)
}
