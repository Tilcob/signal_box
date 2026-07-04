//! Track-drag drawing: turn a dragged cell path into track pieces (interior
//! pieces replace, endpoints yield to existing track), or a single click into
//! the current R/T-rotated form.

use stellwerk_sim::grid::{Cell, Dir8, pair_len};
use stellwerk_sim::layout::{Layout, TrackPiece};
use stellwerk_sim::level::Level;

use super::commit::place_replacing;
use crate::editor::ops::{EditOp, Element, do_op};
use crate::editor::placement::{Placement, plan_piece};
use crate::state::Editor;

/// Interior cells of the drag path get the piece connecting entry and exit
/// direction; the two endpoints (each knows only one drag direction) get a
/// straight piece along it, so a free draw is continuous from the very first
/// cell. An endpoint that sits on existing track (a source/sink anchor) makes
/// the straight clash, so it is skipped and the cell is left to its existing
/// piece — the anchor→anchor campaign drag is unchanged. Returns `true` when at
/// least one piece was actually placed (so the caller plays the build sound
/// only on a real placement, not an empty/blocked drag).
pub(super) fn finish_track_drag(editor: &mut Editor, level: &mut Level, path: &[Cell]) -> bool {
    let dir_between = |from: Cell, to: Cell| -> Option<Dir8> {
        let delta = (to.x - from.x, to.y - from.y);
        Dir8::ALL.into_iter().find(|d| d.cell_offset() == delta)
    };

    let mut ops = Vec::new();
    let mut placed: Vec<TrackPiece> = Vec::new();
    let mut replaced = std::collections::BTreeSet::new();
    for window in path.windows(3) {
        let (prev, cur, next) = (window[0], window[1], window[2]);
        let (Some(entry), Some(exit)) = (dir_between(cur, prev), dir_between(cur, next)) else {
            continue; // cursor jumped more than one cell
        };
        if pair_len(entry, exit).is_none() {
            continue; // kink — silently skip while drawing
        }
        let (a, b) = if entry.index() <= exit.index() {
            (entry, exit)
        } else {
            (exit, entry)
        };
        // Interior pieces are deliberate strokes: drawing over the player's own
        // track replaces it (`allow_replace`), which is how a misbuild is fixed.
        emit_piece(level, &editor.layout, TrackPiece { cell: cur, a, b }, true, &mut ops, &mut placed, &mut replaced);
    }

    // The windows(3) pass never makes the endpoints a `cur`, so they stay empty.
    // Give each a straight piece along its single drag direction — but with
    // `allow_replace = false`: an endpoint sitting on existing track (an anchor,
    // or the player's own line) is left intact, so a drag started from track
    // continues it rather than bulldozing it.
    if path.len() >= 2 {
        for (cell, toward) in [
            (path[0], path[1]),
            (path[path.len() - 1], path[path.len() - 2]),
        ] {
            let Some(dir) = dir_between(cell, toward) else {
                continue; // cursor jumped — no single-step direction
            };
            let opp = dir.opposite();
            let (a, b) = if dir.index() <= opp.index() { (dir, opp) } else { (opp, dir) };
            emit_piece(level, &editor.layout, TrackPiece { cell, a, b }, false, &mut ops, &mut placed, &mut replaced);
        }
    }

    if ops.is_empty() && path.len() == 1 {
        // No drag: click places the current R/T-rotated form, replacing the
        // player's own track underneath if it clashes.
        let (a, b) = editor.track_form;
        let piece = TrackPiece { cell: path[0], a, b };
        let plan = plan_piece(level, &editor.layout, &piece);
        if matches!(plan, Placement::Blocked) {
            return false;
        }
        place_replacing(editor, level, plan, Element::Piece(piece));
        return true;
    }
    if !ops.is_empty() {
        do_op(editor, level, EditOp::Group(ops));
        return true;
    }
    false
}

/// Collects the ops for one drag piece into `ops`. Skips a self-overlap against
/// pieces placed earlier in this same stroke (`placed`). On a clash with the
/// player's own track it either replaces it (`allow_replace`, interior strokes)
/// or skips (endpoints). Each replaced cell is recorded in `replaced` so a
/// self-crossing stroke removes a given piece at most once — emitting the same
/// removal twice would re-add a duplicate when the group is undone.
#[allow(clippy::too_many_arguments)]
fn emit_piece(
    level: &Level,
    layout: &Layout,
    piece: TrackPiece,
    allow_replace: bool,
    ops: &mut Vec<EditOp>,
    placed: &mut Vec<TrackPiece>,
    replaced: &mut std::collections::BTreeSet<Cell>,
) {
    let drag_conflict = placed
        .iter()
        .any(|p| p.cell == piece.cell && [p.a, p.b].iter().any(|d| *d == piece.a || *d == piece.b));
    if drag_conflict {
        return;
    }
    match plan_piece(level, layout, &piece) {
        Placement::Blocked => {}
        Placement::Place => {
            placed.push(piece);
            ops.push(EditOp::Place(Element::Piece(piece)));
        }
        Placement::Replace(old) if allow_replace => {
            if replaced.insert(piece.cell) {
                ops.extend(old.into_iter().map(EditOp::Remove));
            }
            placed.push(piece);
            ops.push(EditOp::Place(Element::Piece(piece)));
        }
        Placement::Replace(_) => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::editor::ops::undo;
    use stellwerk_sim::level::Par;
    use stellwerk_sim::units::Tick;

    fn buildable_row(n: i32) -> Level {
        Level {
            name: String::new(),
            buildable: (0..n).map(|x| Cell { x, y: 0 }).collect(),
            fixed: Layout::default(),
            sources: Vec::new(),
            sinks: Vec::new(),
            schedule: Vec::new(),
            par: Par { throughput: Tick(0), material: 0, lateness: 0 },
        }
    }

    fn row(xs: &[i32]) -> Vec<Cell> {
        xs.iter().map(|&x| Cell { x, y: 0 }).collect()
    }

    /// Free draw over empty buildable cells fills EVERY crossed cell, including
    /// the first and last — the old gap at the drag start is closed.
    #[test]
    fn free_drag_fills_both_endpoints() {
        let mut editor = Editor::default();
        let mut level = buildable_row(4);
        assert!(finish_track_drag(&mut editor, &mut level, &row(&[0, 1, 2, 3])));
        let mut xs: Vec<i32> = editor.layout.pieces.iter().map(|p| p.cell.x).collect();
        xs.sort();
        assert_eq!(xs, vec![0, 1, 2, 3], "start and end cells get a piece too");
    }

    /// A drag starting on existing track (an anchor) leaves that cell to the
    /// anchor — the straight clashes and is skipped, so the campaign
    /// anchor→anchor flow is unchanged. The far (empty) endpoint still fills.
    #[test]
    fn drag_skips_endpoint_on_existing_track() {
        let mut editor = Editor::default();
        let mut level = buildable_row(4);
        level.fixed.pieces.push(TrackPiece { cell: Cell { x: 0, y: 0 }, a: Dir8::W, b: Dir8::E });
        finish_track_drag(&mut editor, &mut level, &row(&[0, 1, 2, 3]));
        assert!(!editor.layout.pieces.iter().any(|p| p.cell.x == 0), "anchored start left open");
        assert!(editor.layout.pieces.iter().any(|p| p.cell.x == 3), "empty end still filled");
    }

    /// Clicking a clashing form over the player's own track replaces it in one
    /// undo step — the old piece comes straight back on undo.
    #[test]
    fn click_replaces_clashing_player_track() {
        let mut editor = Editor::default();
        let mut level = buildable_row(2);
        editor.layout.pieces.push(TrackPiece { cell: Cell { x: 0, y: 0 }, a: Dir8::W, b: Dir8::E });
        editor.track_form = (Dir8::W, Dir8::N); // shares the W connector → clashes
        assert!(finish_track_drag(&mut editor, &mut level, &row(&[0])));
        let on_cell: Vec<_> = editor.layout.pieces.iter().filter(|p| p.cell.x == 0).collect();
        assert_eq!(on_cell.len(), 1, "old clashing piece replaced, not stacked");
        assert_eq!((on_cell[0].a, on_cell[0].b), (Dir8::W, Dir8::N));
        undo(&mut editor, &mut level);
        let restored: Vec<_> = editor.layout.pieces.iter().filter(|p| p.cell.x == 0).collect();
        assert_eq!(restored.len(), 1, "undo restores exactly the original");
        assert_eq!((restored[0].a, restored[0].b), (Dir8::W, Dir8::E));
    }
}
