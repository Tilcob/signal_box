//! Block and erase strokes: a click or drag over a cell path, applied as one
//! undo step. The first cell of a block stroke picks carve-vs-restore; erase
//! peels the topmost element per cell.

use bevy::prelude::*;
use stellwerk_sim::grid::{Cell, Dir8};
use stellwerk_sim::layout::Layout;
use stellwerk_sim::level::Level;

use crate::board;
use crate::editor::ops::{EditOp, Element, do_op};
use crate::editor::placement::{EraseTarget, can_block_cell, erase_target};
use crate::state::{ActiveLevel, Editor};

/// Applies a Block-tool stroke. The first cell picks the mode for the whole
/// stroke: starting on an empty buildable cell carves blocks, starting on an
/// existing hole restores them. Cells ineligible for the chosen mode are
/// skipped, and the stroke is one undo step. Returns whether anything changed.
pub(super) fn apply_block_stroke(editor: &mut Editor, level: &mut Level, merged: &Layout, path: &[Cell]) -> bool {
    let Some(&first) = path.first() else {
        return false;
    };
    let carve = can_block_cell(level, merged, first);
    let holes = board::blocked_cells(&level.buildable);
    let mut ops = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    for &cell in path {
        if !seen.insert(cell) {
            continue;
        }
        if carve {
            if can_block_cell(level, merged, cell) {
                ops.push(EditOp::SetBuildable { cell, on: false });
            }
        } else if holes.contains(&cell) {
            ops.push(EditOp::SetBuildable { cell, on: true });
        }
    }
    match ops.len() {
        0 => false,
        1 => {
            do_op(editor, level, ops.pop().expect("len 1"));
            true
        }
        _ => {
            do_op(editor, level, EditOp::Group(ops));
            true
        }
    }
}

/// The op that erases the topmost element at `(cell, at)`, in the priority
/// order encoded by [`erase_target`]. `None` if the cell holds nothing
/// erasable. Pure — reads state and builds the op so both a click and a drag
/// stroke can collect ops and apply them as one undo step. Designer track lives
/// in `level.fixed`, never in `editor.layout`, so it is never a target here:
/// untouchable.
fn erase_op(editor: &Editor, active: &ActiveLevel, cell: Cell, at: Dir8) -> Option<EditOp> {
    Some(match erase_target(&editor.layout, &active.level, active.sandbox, cell, at)? {
        // A sandbox block holds nothing else, so erasing it just restores
        // buildability (campaign holes are authored shape, never targeted here).
        EraseTarget::Block(cell) => EditOp::SetBuildable { cell, on: true },
        EraseTarget::Source(source) => EditOp::RemoveSource(source),
        EraseTarget::Sink(sink) => {
            // Schedule entries pointing at the removed sink would be a permanent
            // validation error — drop them too. Bundle them with the sink into
            // one Group (rows highest-first so earlier removals don't shift the
            // later indices) so a single undo restores all of it.
            let mut ops: Vec<EditOp> = active
                .level
                .schedule
                .iter()
                .enumerate()
                .filter(|(_, e)| e.sink == sink.id)
                .map(|(row, e)| EditOp::ScheduleRemove { row, entry: e.clone() })
                .collect();
            ops.reverse();
            ops.push(EditOp::RemoveSink(sink));
            EditOp::Group(ops)
        }
        EraseTarget::Signal(signal) => EditOp::Remove(Element::Signal(signal)),
        EraseTarget::Switch(switch) => EditOp::Remove(Element::Switch(switch)),
        EraseTarget::Piece(piece) => EditOp::Remove(Element::Piece(piece)),
    })
}

/// Erase stroke shared by click and drag. A click is a one-cell stroke using
/// the actual `cursor` (so the connector-nearest signal is targeted precisely);
/// a multi-cell drag uses each cell's center connector. One element per cell, in
/// the same priority order as a click — wipe again to peel a second layer. The
/// whole stroke is one undo step (mirrors Track/Block).
pub(super) fn apply_erase_stroke(editor: &mut Editor, active: &mut ActiveLevel, path: &[Cell], cursor: Vec2) {
    let single = path.len() == 1;
    let mut ops = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    for &cell in path {
        if !seen.insert(cell) {
            continue;
        }
        let pos = if single { cursor } else { board::cell_world(cell) };
        let at = board::nearest_connector(cell, pos);
        let Some(op) = erase_op(editor, active, cell, at) else {
            continue;
        };
        // Drop a selection that pointed at a now-erased element, else its panel
        // lingers on something gone (mirrors the old single-click reset).
        match &op {
            EditOp::Remove(Element::Signal(s)) if editor.selected_signal == Some((s.cell, s.at)) => {
                editor.selected_signal = None;
            }
            EditOp::Remove(Element::Switch(s)) if editor.selected_switch == Some(s.cell) => {
                editor.selected_switch = None;
            }
            _ => {}
        }
        ops.push(op);
    }
    match ops.len() {
        0 => {}
        1 => do_op(editor, &mut active.level, ops.pop().expect("len 1")),
        _ => do_op(editor, &mut active.level, EditOp::Group(ops)),
    }
}
