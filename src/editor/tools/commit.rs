//! Shared placement commit: turn a [`Placement`] plan into edit ops, clearing a
//! selection that pointed at whatever gives way. Used by both the pointer
//! (switch placement) and the track single-click.

use stellwerk_sim::level::Level;

use crate::editor::ops::{EditOp, Element, do_op};
use crate::editor::placement::Placement;
use crate::state::Editor;

/// Commits a placement plan: a plain `Place`, or a `Replace` that first removes
/// the player elements giving way — clearing any selection pointing at them —
/// and places the new element in the SAME undo step. `Blocked` is filtered by
/// the caller and is a no-op here.
pub(super) fn place_replacing(editor: &mut Editor, level: &mut Level, plan: Placement, new: Element) {
    match plan {
        Placement::Blocked => {}
        Placement::Place => do_op(editor, level, EditOp::Place(new)),
        Placement::Replace(old) => {
            clear_selection_for(editor, &old);
            let mut ops: Vec<EditOp> = old.into_iter().map(EditOp::Remove).collect();
            ops.push(EditOp::Place(new));
            do_op(editor, level, EditOp::Group(ops));
        }
    }
}

/// Drops a switch/signal selection pointing at an element about to be removed,
/// so its config panel doesn't linger on something gone (mirrors the erase
/// stroke's cleanup).
fn clear_selection_for(editor: &mut Editor, removed: &[Element]) {
    for e in removed {
        match e {
            Element::Switch(s) if editor.selected_switch == Some(s.cell) => {
                editor.selected_switch = None;
            }
            Element::Signal(s) if editor.selected_signal == Some((s.cell, s.at)) => {
                editor.selected_signal = None;
            }
            _ => {}
        }
    }
}
