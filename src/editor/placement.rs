//! Placement variants (R-cycled presets) and placement gates.
//!
//! Hard placement rules (occupied/off-board) are rejected at the tool
//! instead of being placed and flagged: stacking a switch on a switch is
//! never a puzzle state worth inspecting. Cross-cell problems (junction
//! without switch, reachability) stay non-modal — they glow as diagnostics.

use std::collections::BTreeSet;
use std::sync::LazyLock;
use stellwerk_sim::grid::{Cell, Dir8, Point, pair_len};
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

/// The connectors of `cell` that already carry track from a NEIGHBOURING cell
/// (a piece/switch stub — or a level source/sink — meeting one of `cell`'s
/// connector points). `cell` itself is excluded: a switch cell is exclusive,
/// so its connectors come entirely from what joins it.
fn connected_dirs(level: &Level, merged: &Layout, cell: Cell) -> Vec<Dir8> {
    let mut points: BTreeSet<Point> = BTreeSet::new();
    for piece in &merged.pieces {
        if piece.cell == cell {
            continue;
        }
        points.insert(piece.cell.connector_point(piece.a));
        points.insert(piece.cell.connector_point(piece.b));
    }
    for sw in &merged.switches {
        if sw.cell == cell {
            continue;
        }
        points.insert(sw.cell.connector_point(sw.stem));
        points.insert(sw.cell.connector_point(sw.branches[0]));
        points.insert(sw.cell.connector_point(sw.branches[1]));
    }
    // An arm may end directly at a source/sink (no piece between it and the
    // switch), so those connector points count as connected track too.
    for s in &level.sources {
        if s.cell != cell {
            points.insert(s.cell.connector_point(s.dir));
        }
    }
    for s in &level.sinks {
        if s.cell != cell {
            points.insert(s.cell.connector_point(s.dir));
        }
    }
    Dir8::ALL
        .into_iter()
        .filter(|&d| points.contains(&cell.connector_point(d)))
        .collect()
}

/// Auto-orientation for a switch placed at `cell`: when exactly three
/// connectors already carry track AND exactly one of them is a legal stem
/// (both others turn ≤90° off it), return that orientation so the player need
/// not aim it by hand. `None` when the junction is ambiguous (several legal
/// stems, e.g. a symmetric T) or not a clean three-way — the caller then falls
/// back to the R/T preset. The straight branch (opposite the stem) is placed
/// first so it becomes the default, matching the preset switches.
pub(super) fn auto_switch_orientation(
    level: &Level,
    merged: &Layout,
    cell: Cell,
) -> Option<(Dir8, [Dir8; 2])> {
    let dirs = connected_dirs(level, merged, cell);
    let [a, b, c] = dirs.as_slice() else {
        return None;
    };
    let (a, b, c) = (*a, *b, *c);
    let legal: Vec<(Dir8, [Dir8; 2])> = [(a, [b, c]), (b, [a, c]), (c, [a, b])]
        .into_iter()
        .filter(|(stem, [x, y])| pair_len(*stem, *x).is_some() && pair_len(*stem, *y).is_some())
        .collect();
    let [(stem, [b0, b1])] = legal.as_slice() else {
        return None; // zero or several legal stems → not unambiguous
    };
    // Default branch = the straight (opposite-stem) one, like the presets.
    if *b1 == stem.opposite() {
        Some((*stem, [*b1, *b0]))
    } else {
        Some((*stem, [*b0, *b1]))
    }
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

    fn piece(c: Cell, a: Dir8, b: Dir8) -> TrackPiece {
        TrackPiece { cell: c, a, b }
    }

    fn layout(pieces: Vec<TrackPiece>) -> Layout {
        Layout {
            pieces,
            switches: vec![],
            signals: vec![],
        }
    }

    /// Level with no stations — switch arms come purely from track pieces.
    fn bare_level() -> Level {
        Level {
            name: String::new(),
            buildable: vec![],
            fixed: Layout::default(),
            sources: vec![],
            sinks: vec![],
            schedule: vec![],
            par: Par {
                throughput: Tick(0),
                material: 0,
                lateness: 0,
            },
        }
    }

    #[test]
    fn auto_orient_unique_stem() {
        // The screenshot junction at (1,-1): a diagonal in from NW (a piece in
        // (0,0) using SE), and the horizontal W↔E (pieces in (0,-1) and (2,-1)).
        // Only stem=E is legal (NW↔W is a kink), so it is chosen automatically.
        let l = layout(vec![
            piece(cell(0, 0), Dir8::W, Dir8::SE),
            piece(cell(0, -1), Dir8::W, Dir8::E),
            piece(cell(2, -1), Dir8::W, Dir8::E),
        ]);
        let (stem, branches) =
            auto_switch_orientation(&bare_level(), &l, cell(1, -1)).expect("one legal stem");
        assert_eq!(stem, Dir8::E);
        assert!(branches.contains(&Dir8::W) && branches.contains(&Dir8::NW));
        assert_eq!(branches[0], Dir8::W, "straight branch is the default");
    }

    #[test]
    fn auto_orient_counts_a_sink_as_an_arm() {
        // Same junction, but the East arm ends directly at a sink at (2,-1).W
        // (no track piece between it and the switch). The sink connector must
        // still count, so stem=E is found.
        let l = layout(vec![
            piece(cell(0, 0), Dir8::W, Dir8::SE),
            piece(cell(0, -1), Dir8::W, Dir8::E),
        ]);
        let mut lvl = bare_level();
        lvl.sinks.push(SinkDef {
            id: SinkId(0),
            cell: cell(2, -1),
            dir: Dir8::W,
            label: "OST".into(),
        });
        let (stem, _) =
            auto_switch_orientation(&lvl, &l, cell(1, -1)).expect("sink arm counts");
        assert_eq!(stem, Dir8::E);
    }

    #[test]
    fn auto_orient_ambiguous_t_junction_is_none() {
        // W/E/N meeting at (1,0): any of the three is a legal stem → ambiguous,
        // so auto-orient declines and the R/T preset is used instead.
        let l = layout(vec![
            piece(cell(0, 0), Dir8::W, Dir8::E),
            piece(cell(2, 0), Dir8::W, Dir8::E),
            piece(cell(1, 1), Dir8::S, Dir8::N),
        ]);
        assert_eq!(auto_switch_orientation(&bare_level(), &l, cell(1, 0)), None);
    }

    #[test]
    fn auto_orient_needs_exactly_three_connectors() {
        let l = layout(vec![piece(cell(0, 0), Dir8::W, Dir8::E)]);
        assert_eq!(auto_switch_orientation(&bare_level(), &l, cell(1, 0)), None);
    }
}
