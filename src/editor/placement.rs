//! Placement variants (R-cycled presets) and placement gates.
//!
//! Hard placement rules (occupied/off-board) are rejected at the tool
//! instead of being placed and flagged: stacking a switch on a switch is
//! never a puzzle state worth inspecting. Cross-cell problems (junction
//! without switch, reachability) stay non-modal — they glow as diagnostics.

use std::collections::BTreeSet;
use std::sync::LazyLock;
use stellwerk_sim::grid::{Cell, Dir8, Point, pair_len};
use stellwerk_sim::layout::{Layout, SignalDef, SwitchDef, TrackPiece};
use stellwerk_sim::level::{Level, PlatformDef, SinkDef, SourceDef};

use super::ops::Element;

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

/// What placing a build element does to what is already on its cell. The
/// player may build straight over their own track/switches — the conflicting
/// pieces are removed in the same undo step — but designer-fixed track is
/// untouchable, so a clash with it blocks the placement outright.
pub(super) enum Placement {
    /// Off-board, or a clash with fixed (untouchable) track — nothing happens.
    Blocked,
    /// The cell is free for this element; just place it.
    Place,
    /// Place, but first remove these player elements occupying the cell.
    Replace(Vec<Element>),
}

/// The player pieces/switches that must give way for `wanted` to sit on `cell`,
/// or [`Placement::Blocked`] when a designer-fixed element clashes (those are
/// never bulldozed). `conflict` decides which pieces clash: a whole-cell taker
/// (a switch) conflicts with everything; a track piece only with shared
/// connectors. A crossing with disjoint connectors is no conflict — it stays.
fn plan(level: &Level, layout: &Layout, cell: Cell, conflict: impl Fn(&TrackPiece) -> bool) -> Placement {
    if !level.buildable.contains(&cell) {
        return Placement::Blocked;
    }
    // A fixed piece in the way (or any fixed switch on the cell) is untouchable.
    let fixed_clash = level.fixed.pieces.iter().any(|p| p.cell == cell && conflict(p))
        || level.fixed.switches.iter().any(|s| s.cell == cell);
    if fixed_clash {
        return Placement::Blocked;
    }
    let mut replace: Vec<Element> = layout
        .pieces
        .iter()
        .filter(|p| p.cell == cell && conflict(p))
        .map(|p| Element::Piece(*p))
        .collect();
    replace.extend(
        layout
            .switches
            .iter()
            .filter(|s| s.cell == cell)
            .map(|s| Element::Switch(s.clone())),
    );
    if replace.is_empty() {
        Placement::Place
    } else {
        Placement::Replace(replace)
    }
}

/// Placement plan for a track piece: pieces sharing a connector clash (a
/// crossing with disjoint connectors does not), and a switch owns the whole
/// cell so it always clashes.
pub(super) fn plan_piece(level: &Level, layout: &Layout, piece: &TrackPiece) -> Placement {
    plan(level, layout, piece.cell, |p| {
        [p.a, p.b].iter().any(|d| *d == piece.a || *d == piece.b)
    })
}

/// Placement plan for a switch: the cell must be exclusive, so every piece and
/// switch already on it clashes.
pub(super) fn plan_switch(level: &Level, layout: &Layout, cell: Cell) -> Placement {
    plan(level, layout, cell, |_| true)
}

/// The single element the erase tool would remove at `(cell, at)`, for the
/// delete preview and the erase op to agree on. Same priority order as the
/// erase op: (sandbox: block → source → sink at the connector) → signal at the
/// connector (else any on the cell) → switch → piece. Only the player layout is
/// considered, so designer-fixed track is never a target — untouchable.
pub(super) enum EraseTarget {
    Piece(TrackPiece),
    Switch(SwitchDef),
    Signal(SignalDef),
    Block(Cell),
    Source(SourceDef),
    Sink(SinkDef),
    Platform(PlatformDef),
}

pub(super) fn erase_target(
    layout: &Layout,
    level: &Level,
    sandbox: bool,
    cell: Cell,
    at: Dir8,
) -> Option<EraseTarget> {
    if sandbox {
        if crate::board::is_blocked(&level.buildable, cell) {
            return Some(EraseTarget::Block(cell));
        }
        if let Some(source) = level.sources.iter().find(|s| s.cell == cell && s.dir == at) {
            return Some(EraseTarget::Source(source.clone()));
        }
        if let Some(sink) = level.sinks.iter().find(|s| s.cell == cell && s.dir == at) {
            return Some(EraseTarget::Sink(sink.clone()));
        }
        if let Some(platform) = level.platforms.iter().find(|p| p.cell == cell && p.dir == at) {
            return Some(EraseTarget::Platform(platform.clone()));
        }
    }
    if let Some(signal) = layout
        .signals
        .iter()
        .find(|s| s.cell == cell && s.at == at)
        .or_else(|| layout.signals.iter().find(|s| s.cell == cell))
    {
        return Some(EraseTarget::Signal(*signal));
    }
    if let Some(switch) = layout.switches.iter().find(|s| s.cell == cell) {
        return Some(EraseTarget::Switch(switch.clone()));
    }
    if let Some(piece) = layout.pieces.iter().find(|p| p.cell == cell) {
        return Some(EraseTarget::Piece(*piece));
    }
    None
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

/// Auto-orientation for a source/sink placed at `cell`: when the cell sits on
/// exactly one straight edge of the buildable bounding box, return the cardinal
/// pointing off the board so the station faces outward without the player
/// aiming it. `None` at a corner (two edges — ambiguous) or in the interior,
/// where the R/T-cycled `station_dir` drives the direction instead. Tested
/// against the bounding box, not mere buildability, so a cell beside an
/// interior hole is not mistaken for an edge and never points into the hole.
pub(super) fn auto_station_orientation(level: &Level, cell: Cell) -> Option<Dir8> {
    let min_x = level.buildable.iter().map(|c| c.x).min()?;
    let max_x = level.buildable.iter().map(|c| c.x).max()?;
    let min_y = level.buildable.iter().map(|c| c.y).min()?;
    let max_y = level.buildable.iter().map(|c| c.y).max()?;
    // Cardinal neighbours that leave the bounding box are real level edges.
    let outward: Vec<Dir8> = [Dir8::N, Dir8::E, Dir8::S, Dir8::W]
        .into_iter()
        .filter(|&d| {
            let n = cell.neighbor(d);
            n.x < min_x || n.x > max_x || n.y < min_y || n.y > max_y
        })
        .collect();
    match outward.as_slice() {
        [d] => Some(*d),
        _ => None, // 0 = interior, 2 = corner, 3 = thin strip → ambiguous
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
    level.buildable.contains(&cell) && connector_free(level, cell, at)
}

/// Sandbox freight platform: same connector rule as a station — a buildable
/// cell's connector not already hosting a source/sink/platform. Like a station
/// it has no track requirement at placement (validation flags an off-track
/// anchor as `PlatformOffTrack`); unlike a station it usually sits mid-line, so
/// its direction comes from the R/T-cycled `station_dir`, not auto-orientation.
pub(super) fn can_place_platform(level: &Level, cell: Cell, at: Dir8) -> bool {
    level.buildable.contains(&cell) && connector_free(level, cell, at)
}

/// True when no source, sink or platform already anchors on `(cell, at)` — a
/// connector hosts at most one such element.
fn connector_free(level: &Level, cell: Cell, at: Dir8) -> bool {
    !level.sources.iter().any(|s| s.cell == cell && s.dir == at)
        && !level.sinks.iter().any(|s| s.cell == cell && s.dir == at)
        && !level.platforms.iter().any(|p| p.cell == cell && p.dir == at)
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
            platforms: vec![],
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

    fn rect_buildable(w: i32, h: i32) -> Vec<Cell> {
        (0..w).flat_map(|x| (0..h).map(move |y| cell(x, y))).collect()
    }

    #[test]
    fn station_auto_faces_off_a_straight_edge() {
        let mut lvl = bare_level();
        lvl.buildable = rect_buildable(3, 3);
        assert_eq!(auto_station_orientation(&lvl, cell(0, 1)), Some(Dir8::W));
        assert_eq!(auto_station_orientation(&lvl, cell(2, 1)), Some(Dir8::E));
        assert_eq!(auto_station_orientation(&lvl, cell(1, 2)), Some(Dir8::N));
        assert_eq!(auto_station_orientation(&lvl, cell(1, 0)), Some(Dir8::S));
    }

    #[test]
    fn station_auto_declines_at_corner_and_interior() {
        let mut lvl = bare_level();
        lvl.buildable = rect_buildable(3, 3);
        assert_eq!(auto_station_orientation(&lvl, cell(0, 0)), None); // corner
        assert_eq!(auto_station_orientation(&lvl, cell(1, 1)), None); // interior
    }

    #[test]
    fn station_auto_ignores_interior_holes() {
        // A hole at (1,1): its neighbours stay inside the bbox, so an adjacent
        // cell is not read as an edge and a real edge still resolves.
        let mut lvl = bare_level();
        let mut cells = rect_buildable(3, 3);
        cells.retain(|c| *c != cell(1, 1));
        lvl.buildable = cells;
        assert_eq!(auto_station_orientation(&lvl, cell(1, 0)), Some(Dir8::S));
        assert_eq!(auto_station_orientation(&lvl, cell(0, 1)), Some(Dir8::W));
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
            platforms: vec![],
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

    fn one_cell() -> Level {
        let mut lvl = bare_level();
        lvl.buildable = vec![cell(0, 0)];
        lvl
    }

    #[test]
    fn plan_piece_places_on_empty_cell() {
        let p = piece(cell(0, 0), Dir8::W, Dir8::E);
        assert!(matches!(plan_piece(&one_cell(), &Layout::default(), &p), Placement::Place));
    }

    #[test]
    fn plan_piece_replaces_player_piece_sharing_connector() {
        let existing = layout(vec![piece(cell(0, 0), Dir8::W, Dir8::E)]);
        // The new piece shares the W connector → the old one gives way.
        let p = piece(cell(0, 0), Dir8::W, Dir8::N);
        let Placement::Replace(old) = plan_piece(&one_cell(), &existing, &p) else {
            panic!("expected replace");
        };
        assert!(matches!(old.as_slice(), [Element::Piece(_)]));
    }

    #[test]
    fn plan_piece_disjoint_crossing_coexists() {
        let existing = layout(vec![piece(cell(0, 0), Dir8::W, Dir8::E)]);
        // N/S over W/E is a legal crossing, not a conflict.
        let p = piece(cell(0, 0), Dir8::N, Dir8::S);
        assert!(matches!(plan_piece(&one_cell(), &existing, &p), Placement::Place));
    }

    #[test]
    fn plan_piece_over_fixed_is_blocked() {
        let mut lvl = one_cell();
        lvl.fixed.pieces.push(piece(cell(0, 0), Dir8::W, Dir8::E));
        let p = piece(cell(0, 0), Dir8::W, Dir8::N);
        assert!(matches!(plan_piece(&lvl, &Layout::default(), &p), Placement::Blocked));
    }

    #[test]
    fn plan_switch_replaces_player_piece_on_cell() {
        let existing = layout(vec![piece(cell(0, 0), Dir8::W, Dir8::E)]);
        let Placement::Replace(old) = plan_switch(&one_cell(), &existing, cell(0, 0)) else {
            panic!("expected replace");
        };
        assert!(matches!(old.as_slice(), [Element::Piece(_)]));
    }

    #[test]
    fn plan_switch_over_fixed_switch_is_blocked() {
        let mut lvl = one_cell();
        lvl.fixed.switches.push(stellwerk_sim::layout::SwitchDef {
            cell: cell(0, 0),
            stem: Dir8::E,
            branches: [Dir8::W, Dir8::NW],
            default_branch: 0,
            rules: vec![],
        });
        assert!(matches!(plan_switch(&lvl, &Layout::default(), cell(0, 0)), Placement::Blocked));
    }

    fn a_switch() -> SwitchDef {
        SwitchDef {
            cell: cell(0, 0),
            stem: Dir8::E,
            branches: [Dir8::W, Dir8::NW],
            default_branch: 0,
            rules: vec![],
        }
    }

    fn a_signal() -> SignalDef {
        SignalDef {
            cell: cell(0, 0),
            at: Dir8::E,
            kind: stellwerk_sim::layout::SignalKind::Block,
            priority: 0,
        }
    }

    #[test]
    fn erase_target_priority_signal_switch_piece() {
        let mut lay = layout(vec![piece(cell(0, 0), Dir8::W, Dir8::E)]);
        lay.switches.push(a_switch());
        lay.signals.push(a_signal());
        let lvl = bare_level();
        assert!(matches!(erase_target(&lay, &lvl, false, cell(0, 0), Dir8::E), Some(EraseTarget::Signal(_))));
        lay.signals.clear();
        assert!(matches!(erase_target(&lay, &lvl, false, cell(0, 0), Dir8::E), Some(EraseTarget::Switch(_))));
        lay.switches.clear();
        assert!(matches!(erase_target(&lay, &lvl, false, cell(0, 0), Dir8::E), Some(EraseTarget::Piece(_))));
        lay.pieces.clear();
        assert!(erase_target(&lay, &lvl, false, cell(0, 0), Dir8::E).is_none());
    }

    #[test]
    fn erase_target_sandbox_source_only_visible_in_sandbox() {
        let mut lay = Layout::default();
        lay.signals.push(a_signal());
        let mut lvl = bare_level();
        lvl.sources.push(SourceDef {
            id: SourceId(0),
            cell: cell(0, 0),
            dir: Dir8::E,
            label: String::new(),
        });
        // Sandbox: the source at the connector outranks the signal.
        assert!(matches!(erase_target(&lay, &lvl, true, cell(0, 0), Dir8::E), Some(EraseTarget::Source(_))));
        // Outside the sandbox the source is not erasable → the signal is targeted.
        assert!(matches!(erase_target(&lay, &lvl, false, cell(0, 0), Dir8::E), Some(EraseTarget::Signal(_))));
    }
}
