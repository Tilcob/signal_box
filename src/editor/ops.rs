//! Invertible edit operations — the undo/redo vocabulary, and the
//! basis of the sharing format.
//!
//! Two edit surfaces share ONE timeline: layout build actions
//! (track/switch/signal, every level) and sandbox-only level edits
//! (sources/sinks/schedule). Both apply through [`EditTarget`] so a single
//! Ctrl+Z unwinds them in order — no second stack to interleave.

use stellwerk_sim::grid::Cell;
use stellwerk_sim::layout::{Layout, SignalDef, SwitchDef, TrackPiece};
use stellwerk_sim::level::{Level, ScheduleEntry, SinkDef, SourceDef};
use stellwerk_sim::units::{SinkId, SourceId};

use crate::state::Editor;

#[derive(Debug, Clone)]
pub enum Element {
    Piece(TrackPiece),
    Switch(SwitchDef),
    Signal(SignalDef),
}

/// Invertible edit action over the layout and (sandbox) the level.
#[derive(Debug, Clone)]
pub enum EditOp {
    Place(Element),
    Remove(Element),
    Configure {
        cell: Cell,
        before: SwitchDef,
        after: SwitchDef,
    },
    // Sandbox-only level edits. Stations carry their full
    // def so Place/Remove invert by value, like the layout `Element` ops.
    PlaceSource(SourceDef),
    RemoveSource(SourceDef),
    PlaceSink(SinkDef),
    RemoveSink(SinkDef),
    // Station rename: only the label changes, so the op carries id + old/new
    // name and inverts by swapping them.
    RenameSource {
        id: SourceId,
        before: String,
        after: String,
    },
    RenameSink {
        id: SinkId,
        before: String,
        after: String,
    },
    // Schedule rows are position-sensitive, so insert/remove carry the row;
    // append is `ScheduleInsert { row: schedule.len(), .. }` at the call site.
    ScheduleInsert {
        row: usize,
        entry: ScheduleEntry,
    },
    ScheduleRemove {
        row: usize,
        entry: ScheduleEntry,
    },
    ScheduleEdit {
        row: usize,
        before: ScheduleEntry,
        after: ScheduleEntry,
    },
    Group(Vec<EditOp>),
}

/// The mutable surfaces an [`EditOp`] applies to: the player layout plus, in
/// the sandbox, the level itself. Campaign ops never touch `level`.
struct EditTarget<'a> {
    layout: &'a mut Layout,
    level: &'a mut Level,
}

/// Removes the FIRST matching element only: duplicates are transiently
/// legal (validation flags them), and removing all copies at once would
/// break Place/Remove inversion symmetry for undo/redo.
fn remove_first<T: PartialEq>(items: &mut Vec<T>, target: &T) {
    if let Some(index) = items.iter().position(|x| x == target) {
        items.remove(index);
    }
}

fn apply(target: &mut EditTarget, op: &EditOp) {
    match op {
        EditOp::Place(Element::Piece(p)) => target.layout.pieces.push(*p),
        EditOp::Place(Element::Switch(s)) => target.layout.switches.push(s.clone()),
        EditOp::Place(Element::Signal(s)) => target.layout.signals.push(*s),
        EditOp::Remove(Element::Piece(p)) => remove_first(&mut target.layout.pieces, p),
        EditOp::Remove(Element::Switch(s)) => remove_first(&mut target.layout.switches, s),
        EditOp::Remove(Element::Signal(s)) => remove_first(&mut target.layout.signals, s),
        EditOp::Configure { cell, after, .. } => {
            if let Some(s) = target.layout.switches.iter_mut().find(|s| s.cell == *cell) {
                *s = after.clone();
            }
        }
        EditOp::PlaceSource(s) => target.level.sources.push(s.clone()),
        EditOp::RemoveSource(s) => remove_first(&mut target.level.sources, s),
        EditOp::PlaceSink(s) => target.level.sinks.push(s.clone()),
        EditOp::RemoveSink(s) => remove_first(&mut target.level.sinks, s),
        EditOp::RenameSource { id, after, .. } => {
            if let Some(s) = target.level.sources.iter_mut().find(|s| s.id == *id) {
                s.label = after.clone();
            }
        }
        EditOp::RenameSink { id, after, .. } => {
            if let Some(s) = target.level.sinks.iter_mut().find(|s| s.id == *id) {
                s.label = after.clone();
            }
        }
        EditOp::ScheduleInsert { row, entry } => {
            let row = (*row).min(target.level.schedule.len());
            target.level.schedule.insert(row, entry.clone());
        }
        EditOp::ScheduleRemove { row, .. } => {
            if *row < target.level.schedule.len() {
                target.level.schedule.remove(*row);
            }
        }
        EditOp::ScheduleEdit { row, after, .. } => {
            if let Some(slot) = target.level.schedule.get_mut(*row) {
                *slot = after.clone();
            }
        }
        EditOp::Group(ops) => {
            for op in ops {
                apply(target, op);
            }
        }
    }
}

fn invert(op: &EditOp) -> EditOp {
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
        EditOp::PlaceSource(s) => EditOp::RemoveSource(s.clone()),
        EditOp::RemoveSource(s) => EditOp::PlaceSource(s.clone()),
        EditOp::PlaceSink(s) => EditOp::RemoveSink(s.clone()),
        EditOp::RemoveSink(s) => EditOp::PlaceSink(s.clone()),
        EditOp::RenameSource { id, before, after } => EditOp::RenameSource {
            id: *id,
            before: after.clone(),
            after: before.clone(),
        },
        EditOp::RenameSink { id, before, after } => EditOp::RenameSink {
            id: *id,
            before: after.clone(),
            after: before.clone(),
        },
        EditOp::ScheduleInsert { row, entry } => EditOp::ScheduleRemove {
            row: *row,
            entry: entry.clone(),
        },
        EditOp::ScheduleRemove { row, entry } => EditOp::ScheduleInsert {
            row: *row,
            entry: entry.clone(),
        },
        EditOp::ScheduleEdit { row, before, after } => EditOp::ScheduleEdit {
            row: *row,
            before: after.clone(),
            after: before.clone(),
        },
        EditOp::Group(ops) => EditOp::Group(ops.iter().rev().map(invert).collect()),
    }
}

/// Applies an op and records it for undo. `level` is mutated only by the
/// sandbox level ops; layout ops leave it untouched. Public for the switch
/// panel and schedule panel (ui).
pub fn do_op(editor: &mut Editor, level: &mut Level, op: EditOp) {
    apply(
        &mut EditTarget {
            layout: &mut editor.layout,
            level,
        },
        &op,
    );
    editor.undo.push(op);
    editor.redo.clear();
}

/// Pops and inverts the last op, moving it to the redo stack.
pub(super) fn undo(editor: &mut Editor, level: &mut Level) {
    if let Some(op) = editor.undo.pop() {
        let inverse = invert(&op);
        apply(
            &mut EditTarget {
                layout: &mut editor.layout,
                level,
            },
            &inverse,
        );
        editor.redo.push(op);
    }
}

/// Re-applies the last undone op, moving it back to the undo stack.
pub(super) fn redo(editor: &mut Editor, level: &mut Level) {
    if let Some(op) = editor.redo.pop() {
        apply(
            &mut EditTarget {
                layout: &mut editor.layout,
                level,
            },
            &op,
        );
        editor.undo.push(op);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use stellwerk_sim::level::Par;
    use stellwerk_sim::units::{Len, SinkId, Speed, Tick, TrainClass, TrainId};

    fn fresh_level() -> Level {
        Level {
            name: String::new(),
            buildable: Vec::new(),
            fixed: Layout::default(),
            sources: Vec::new(),
            sinks: Vec::new(),
            schedule: Vec::new(),
            par: Par {
                throughput: Tick(0),
                material: 0,
                lateness: 0,
            },
        }
    }

    fn entry(train: u32, sink: u32) -> ScheduleEntry {
        ScheduleEntry {
            train: TrainId(train),
            class: TrainClass(0),
            length: Len(800),
            speed: Speed(100),
            source: stellwerk_sim::units::SourceId(0),
            sink: SinkId(sink),
            depart: Tick(0),
            due: Tick(80),
        }
    }

    fn source(id: u32) -> SourceDef {
        SourceDef {
            id: stellwerk_sim::units::SourceId(id),
            cell: Cell { x: id as i32, y: 0 },
            dir: stellwerk_sim::grid::Dir8::E,
            label: String::new(),
        }
    }

    fn sink(id: u32) -> SinkDef {
        SinkDef {
            id: SinkId(id),
            cell: Cell { x: id as i32, y: 1 },
            dir: stellwerk_sim::grid::Dir8::W,
            label: format!("Z{id}"),
        }
    }

    /// Applies `ops` in order, then their inverses in reverse — the level must
    /// return to its starting state. Schedule order is position-sensitive, so
    /// this checks exact equality, not just membership.
    fn assert_roundtrip(level: &mut Level, ops: Vec<EditOp>) {
        let before = level.clone();
        let mut layout = Layout::default();
        let mut applied = Vec::new();
        for op in ops {
            apply(
                &mut EditTarget {
                    layout: &mut layout,
                    level,
                },
                &op,
            );
            applied.push(op);
        }
        for op in applied.iter().rev() {
            let inverse = invert(op);
            apply(
                &mut EditTarget {
                    layout: &mut layout,
                    level,
                },
                &inverse,
            );
        }
        assert_eq!(level.schedule, before.schedule, "schedule drift");
        assert_eq!(level.sources, before.sources, "sources drift");
        assert_eq!(level.sinks, before.sinks, "sinks drift");
    }

    #[test]
    fn schedule_insert_edit_remove_roundtrip() {
        let mut level = fresh_level();
        level.sinks.push(sink(0));
        level.schedule.push(entry(0, 0));
        level.schedule.push(entry(1, 0));
        assert_roundtrip(
            &mut level,
            vec![
                EditOp::ScheduleInsert {
                    row: 1,
                    entry: entry(9, 0),
                },
                EditOp::ScheduleEdit {
                    row: 0,
                    before: entry(0, 0),
                    after: entry(0, 0),
                },
                EditOp::ScheduleRemove {
                    row: 2,
                    entry: entry(1, 0),
                },
            ],
        );
    }

    #[test]
    fn station_place_remove_roundtrip() {
        let mut level = fresh_level();
        level.sources.push(source(0));
        level.sinks.push(sink(0));
        assert_roundtrip(
            &mut level,
            vec![
                EditOp::PlaceSource(source(1)),
                EditOp::PlaceSink(sink(1)),
                EditOp::RemoveSource(source(0)),
            ],
        );
    }

    #[test]
    fn station_rename_roundtrip() {
        let mut level = fresh_level();
        level.sources.push(source(0));
        level.sinks.push(sink(0));
        assert_roundtrip(
            &mut level,
            vec![
                EditOp::RenameSource {
                    id: stellwerk_sim::units::SourceId(0),
                    before: String::new(),
                    after: "NORD".into(),
                },
                EditOp::RenameSink {
                    id: SinkId(0),
                    before: "Z0".into(),
                    after: "OST".into(),
                },
            ],
        );
    }

    /// Sink removal cascades its dependent schedule rows in a Group; one undo
    /// must restore both the sink and the rows, in their original positions.
    #[test]
    fn sink_removal_group_roundtrip() {
        let mut level = fresh_level();
        level.sinks.push(sink(0));
        level.sinks.push(sink(1));
        level.schedule.push(entry(0, 0));
        level.schedule.push(entry(1, 1)); // depends on sink 1
        level.schedule.push(entry(2, 0));
        assert_roundtrip(
            &mut level,
            vec![EditOp::Group(vec![
                EditOp::ScheduleRemove {
                    row: 1,
                    entry: entry(1, 1),
                },
                EditOp::RemoveSink(sink(1)),
            ])],
        );
    }
}
