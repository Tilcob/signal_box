//! Invertible edit operations — the undo/redo vocabulary, and in M2 the
//! basis of the sharing format.

use stellwerk_sim::grid::Cell;
use stellwerk_sim::layout::{Layout, SignalDef, SwitchDef, TrackPiece};

use crate::state::Editor;

#[derive(Debug, Clone)]
pub enum Element {
    Piece(TrackPiece),
    Switch(SwitchDef),
    Signal(SignalDef),
}

/// Invertible build action.
#[derive(Debug, Clone)]
pub enum EditOp {
    Place(Element),
    Remove(Element),
    Configure {
        cell: Cell,
        before: SwitchDef,
        after: SwitchDef,
    },
    Group(Vec<EditOp>),
}

/// Removes the FIRST matching element only: duplicates are transiently
/// legal (validation flags them), and removing all copies at once would
/// break Place/Remove inversion symmetry for undo/redo.
fn remove_first<T: PartialEq>(items: &mut Vec<T>, target: &T) {
    if let Some(index) = items.iter().position(|x| x == target) {
        items.remove(index);
    }
}

pub(super) fn apply(layout: &mut Layout, op: &EditOp) {
    match op {
        EditOp::Place(Element::Piece(p)) => layout.pieces.push(*p),
        EditOp::Place(Element::Switch(s)) => layout.switches.push(s.clone()),
        EditOp::Place(Element::Signal(s)) => layout.signals.push(*s),
        EditOp::Remove(Element::Piece(p)) => remove_first(&mut layout.pieces, p),
        EditOp::Remove(Element::Switch(s)) => remove_first(&mut layout.switches, s),
        EditOp::Remove(Element::Signal(s)) => remove_first(&mut layout.signals, s),
        EditOp::Configure { cell, after, .. } => {
            if let Some(s) = layout.switches.iter_mut().find(|s| s.cell == *cell) {
                *s = after.clone();
            }
        }
        EditOp::Group(ops) => {
            for op in ops {
                apply(layout, op);
            }
        }
    }
}

pub(super) fn invert(op: &EditOp) -> EditOp {
    match op {
        EditOp::Place(e) => EditOp::Remove(e.clone()),
        EditOp::Remove(e) => EditOp::Place(e.clone()),
        EditOp::Configure {
            cell,
            before,
            after,
        } => EditOp::Configure {
            cell: *cell,
            before: after.clone(),
            after: before.clone(),
        },
        EditOp::Group(ops) => EditOp::Group(ops.iter().rev().map(invert).collect()),
    }
}

/// Applies an op and records it for undo. Public for the switch panel (ui).
pub fn do_op(editor: &mut Editor, op: EditOp) {
    apply(&mut editor.layout, &op);
    editor.undo.push(op);
    editor.redo.clear();
}
