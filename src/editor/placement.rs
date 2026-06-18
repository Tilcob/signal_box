//! Placement variants (R-cycled presets) and placement gates.
//!
//! Hard placement rules (occupied/off-board) are rejected at the tool
//! instead of being placed and flagged: stacking a switch on a switch is
//! never a puzzle state worth inspecting. Cross-cell problems (junction
//! without switch, reachability) stay non-modal — they glow as diagnostics.

use std::sync::LazyLock;
use stellwerk_sim::grid::{Cell, Dir8};
use stellwerk_sim::layout::{Layout, TrackPiece};
use stellwerk_sim::level::Level;

/// 8 switch presets: cardinal stem, branches = straight-through + 45° turn.
/// Fixed table — built once, not per frame (this is read in the overlay draw,
/// which runs every frame while the switch tool is active).
pub(super) fn switch_variants() -> &'static [(Dir8, [Dir8; 2])] {
    static VARIANTS: LazyLock<Vec<(Dir8, [Dir8; 2])>> = LazyLock::new(|| {
        let rot = |d: Dir8, k: usize| Dir8::ALL[(d.index() as usize + k) % 8];
        let mut out = Vec::new();
        for stem in [Dir8::W, Dir8::E, Dir8::N, Dir8::S] {
            let straight = stem.opposite();
            out.push((stem, [straight, rot(straight, 1)]));
            out.push((stem, [straight, rot(straight, 7)]));
        }
        out
    });
    &VARIANTS
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

/// A cell may be blocked (turned non-buildable) only if it is currently
/// buildable and completely empty — blocking never silently deletes the
/// player's track, switches, signals or a station sitting on it.
pub(super) fn can_block_cell(level: &Level, merged: &Layout, cell: Cell) -> bool {
    level.buildable.contains(&cell)
        && !merged.pieces.iter().any(|p| p.cell == cell)
        && !merged.switches.iter().any(|s| s.cell == cell)
        && !merged.signals.iter().any(|s| s.cell == cell)
        && !level.sources.iter().any(|s| s.cell == cell)
        && !level.sinks.iter().any(|s| s.cell == cell)
}

/// Switch cells are exclusive: buildable and completely empty.
pub(super) fn can_place_switch(level: &Level, merged: &Layout, cell: Cell) -> bool {
    level.buildable.contains(&cell)
        && !merged.pieces.iter().any(|p| p.cell == cell)
        && !merged.switches.iter().any(|s| s.cell == cell)
}

/// The signal anchor chosen by the R/T-cycled `variant` among the connectors
/// of `cell` that actually carry track. `None` for a cell with no track —
/// the mouse only picks the cell, the direction is keyboard-driven.
pub(super) fn signal_stub(merged: &Layout, cell: Cell, variant: i32) -> Option<Dir8> {
    // Pick the nth matching stub without collecting into a Vec — this runs
    // every frame while the signal tool is active, and Dir8::ALL is only 8.
    let count = Dir8::ALL.iter().filter(|&&d| merged.has_stub(cell, d)).count();
    if count == 0 {
        return None;
    }
    let nth = variant.rem_euclid(count as i32) as usize;
    Dir8::ALL
        .into_iter()
        .filter(|&d| merged.has_stub(cell, d))
        .nth(nth)
}

/// Signals need track under their connector and may not stack.
pub(super) fn can_place_signal(level: &Level, merged: &Layout, cell: Cell, at: Dir8) -> bool {
    level.buildable.contains(&cell)
        && merged.has_stub(cell, at)
        && !merged.signals.iter().any(|s| s.cell == cell && s.at == at)
}

/// The station connector chosen by the R/T-cycled `variant`. Stations have no
/// track requirement (unlike signals), so the mouse only picks the cell and
/// the direction cycles through all 8 connectors via R/T — never the cursor.
pub(super) fn station_dir(variant: i32) -> Dir8 {
    Dir8::ALL[variant.rem_euclid(8) as usize]
}

/// Sandbox sources/sinks: a buildable cell's connector, not already occupied
/// by another station (a connector hosts at most one entry/exit). Like the
/// other gates this is enforced at the tool, not left for validation.
pub(super) fn can_place_station(level: &Level, cell: Cell, at: Dir8) -> bool {
    level.buildable.contains(&cell)
        && !level.sources.iter().any(|s| s.cell == cell && s.dir == at)
        && !level.sinks.iter().any(|s| s.cell == cell && s.dir == at)
}

#[cfg(test)]
mod tests {
    use super::*;
    use stellwerk_sim::level::{Level, Par, SinkDef, SourceDef};
    use stellwerk_sim::units::{SinkId, SourceId, Tick};

    fn cell(x: i32, y: i32) -> Cell {
        Cell { x, y }
    }

    fn level() -> Level {
        Level {
            name: "t".into(),
            buildable: vec![cell(0, 0), cell(1, 0)],
            fixed: Layout::default(),
            sources: vec![SourceDef {
                id: SourceId(0),
                cell: cell(0, 0),
                dir: Dir8::W,
                label: String::new(),
            }],
            sinks: vec![SinkDef {
                id: SinkId(0),
                cell: cell(1, 0),
                dir: Dir8::E,
                label: "OST".into(),
            }],
            schedule: vec![],
            par: Par {
                throughput: Tick(0),
                material: 0,
                lateness: 0,
            },
        }
    }

    #[test]
    fn station_only_on_buildable() {
        let lvl = level();
        assert!(can_place_station(&lvl, cell(1, 0), Dir8::N));
        // Outside the buildable strip is rejected.
        assert!(!can_place_station(&lvl, cell(5, 5), Dir8::W));
    }

    #[test]
    fn station_connector_not_double_booked() {
        let lvl = level();
        // The existing source sits on (0,0) W and the sink on (1,0) E.
        assert!(!can_place_station(&lvl, cell(0, 0), Dir8::W));
        assert!(!can_place_station(&lvl, cell(1, 0), Dir8::E));
        // A free connector on the same buildable cell is fine.
        assert!(can_place_station(&lvl, cell(0, 0), Dir8::E));
    }
}
