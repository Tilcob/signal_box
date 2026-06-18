//! Tool input: hotkeys (incl. layout-aware undo/redo), the board pointer
//! with placement/erase, track drags and leaving back to the level select.

use bevy::input::keyboard::Key;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use stellwerk_sim::grid::{Cell, Dir8, pair_len};
use stellwerk_sim::layout::{Layout, SignalDef, SignalKind, SwitchDef, TrackPiece};
use stellwerk_sim::level::Level;
use stellwerk_sim::units::{SinkId, SourceId};

use super::ops::{EditOp, Element, do_op, redo, undo};
use super::placement::{
    auto_switch_orientation, can_block_cell, can_place_piece, can_place_signal, can_place_station,
    can_place_switch, signal_stub, station_dir, switch_variants,
};
use crate::board;
use crate::camera::{MainCamera, cursor_world};
use crate::state::{ActiveLevel, Editor, Tool};

pub(super) fn hotkeys(
    keys: Res<ButtonInput<KeyCode>>,
    logical: Res<ButtonInput<Key>>,
    active: Option<ResMut<ActiveLevel>>,
    mut editor: ResMut<Editor>,
) {
    let mut active = active;
    let sandbox = active.as_ref().is_some_and(|a| a.sandbox);
    let bypass = editor.bypass_change_detection();
    if keys.just_pressed(KeyCode::KeyQ) {
        bypass.tool = Tool::Select;
    }
    if keys.just_pressed(KeyCode::Digit1) {
        bypass.tool = Tool::Track;
    }
    if keys.just_pressed(KeyCode::Digit2) {
        bypass.tool = Tool::Switch;
    }
    if keys.just_pressed(KeyCode::Digit3) {
        bypass.tool = Tool::SignalBlock;
    }
    if keys.just_pressed(KeyCode::Digit4) {
        bypass.tool = Tool::SignalChain;
    }
    if keys.just_pressed(KeyCode::KeyB) {
        bypass.tool = Tool::Erase;
    }
    if sandbox && keys.just_pressed(KeyCode::Digit5) {
        bypass.tool = Tool::Block;
    }
    if sandbox && keys.just_pressed(KeyCode::Digit6) {
        bypass.tool = Tool::Source;
    }
    if sandbox && keys.just_pressed(KeyCode::Digit7) {
        bypass.tool = Tool::Sink;
    }
    // R = rotate left (−45°), T = rotate right (+45°) for every tool. Tracks
    // rotate their whole form through the 8 orientations; switch/signal rotate
    // their variant counter.
    let r = keys.just_pressed(KeyCode::KeyR);
    let t = keys.just_pressed(KeyCode::KeyT);
    if r ^ t {
        let steps = if r { -1 } else { 1 };
        match bypass.tool {
            Tool::Track => {
                let (a, b) = bypass.track_form;
                bypass.track_form = (a.rotate(steps), b.rotate(steps));
            }
            _ => bypass.variant += steps,
        }
    }

    // Undo/redo match the LOGICAL key, not the physical KeyCode: KeyCode
    // names US positions, so on a German QWERTZ layout the key labeled "Z"
    // arrives as KeyCode::KeyY — Ctrl+Z would silently trigger redo.
    let chr = |s: &str| Key::Character(s.into());
    let ctrl = keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight);
    let undo_pressed = logical.just_pressed(chr("z")) || logical.just_pressed(chr("Z"));
    let redo_pressed = logical.just_pressed(chr("y")) || logical.just_pressed(chr("Y"));
    // undo/redo replay layout AND sandbox level ops, so they need the level.
    // `&mut editor`/`&mut active.level` re-borrow WITH change detection on
    // purpose: the merged layout and schedule panel must rebuild afterwards.
    if ctrl && undo_pressed && let Some(active) = active.as_deref_mut() {
        undo(&mut editor, &mut active.level);
    }
    if ctrl && redo_pressed && let Some(active) = active.as_deref_mut() {
        redo(&mut editor, &mut active.level);
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn pointer(
    buttons: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    cameras: Query<(&Camera, &GlobalTransform), With<MainCamera>>,
    ui: Query<&Interaction>,
    active: Option<ResMut<ActiveLevel>>,
    mut editor: ResMut<Editor>,
    merged: Res<super::MergedLayout>,
    mut commands: Commands,
) {
    let Some(mut active) = active else { return };
    let Some(cursor) = cursor_world(&windows, &cameras) else {
        return;
    };
    // Clicks that land on UI (switch panel, slot buttons, start button)
    // must not fall through to the board: clicking the just-opened switch
    // panel would deselect the switch again, and tools would edit track
    // hidden behind the panel.
    let over_ui = ui.iter().any(|i| *i != Interaction::None);
    let cell = board::world_cell(cursor);
    let merged = &merged.0;

    // While the radial track menu is open it owns the mouse — placement and
    // drag are suppressed (handled by `radial::radial_menu`, which runs after
    // this so it sees the menu still open on the click frame).
    if editor.radial.is_some() {
        return;
    }

    // Track drag: collect cells while held.
    if editor.tool == Tool::Track {
        if buttons.just_pressed(MouseButton::Left) && !over_ui {
            editor.bypass_change_detection().drag = Some(vec![cell]);
        }
        if buttons.pressed(MouseButton::Left) {
            let bypass = editor.bypass_change_detection();
            if let Some(path) = &mut bypass.drag
                && path.last() != Some(&cell)
            {
                path.push(cell);
            }
        }
        if buttons.just_released(MouseButton::Left) {
            let path = editor
                .bypass_change_detection()
                .drag
                .take()
                .unwrap_or_default();
            if finish_track_drag(&mut editor, &mut active.level, merged, &path) {
                commands.trigger(crate::audio::SfxKind::BuildingSound);
            }
        }
        return;
    }

    // Block tool (sandbox): drag-paint cells non-buildable, or restore holes.
    // Like the track tool it collects a path; the first cell picks the mode.
    if editor.tool == Tool::Block && active.sandbox {
        if buttons.just_pressed(MouseButton::Left) && !over_ui {
            editor.bypass_change_detection().drag = Some(vec![cell]);
        }
        if buttons.pressed(MouseButton::Left) {
            let bypass = editor.bypass_change_detection();
            if let Some(path) = &mut bypass.drag
                && path.last() != Some(&cell)
            {
                path.push(cell);
            }
        }
        if buttons.just_released(MouseButton::Left) {
            let path = editor
                .bypass_change_detection()
                .drag
                .take()
                .unwrap_or_default();
            if apply_block_stroke(&mut editor, &mut active.level, merged, &path) {
                commands.trigger(crate::audio::SfxKind::BuildingSound);
            }
        }
        return;
    }

    if !buttons.just_pressed(MouseButton::Left) || over_ui {
        return;
    }

    match editor.tool {
        Tool::Track => unreachable!("handled above"),
        Tool::Switch => {
            if !can_place_switch(&active.level, merged, cell) {
                return;
            }
            // Auto-orient from the surrounding track when the junction admits
            // exactly one legal stem (the player needn't aim it); otherwise the
            // R/T-cycled preset.
            let (stem, branches) = auto_switch_orientation(&active.level, merged, cell).unwrap_or_else(|| {
                let variants = switch_variants();
                variants[editor.variant.rem_euclid(variants.len() as i32) as usize]
            });
            do_op(
                &mut editor,
                &mut active.level,
                EditOp::Place(Element::Switch(SwitchDef {
                    cell,
                    stem,
                    branches,
                    default_branch: 0,
                    rules: vec![],
                })),
            );
            commands.trigger(crate::audio::SfxKind::BuildingSound);
        }
        Tool::SignalBlock | Tool::SignalChain => {
            // Direction is keyboard-driven (R/T cycle the cell's stubs), not
            // taken from the cursor angle.
            let Some(at) = signal_stub(merged, cell, editor.variant) else {
                return;
            };
            if !can_place_signal(&active.level, merged, cell, at) {
                return;
            }
            let kind = if editor.tool == Tool::SignalBlock {
                SignalKind::Block
            } else {
                SignalKind::Chain
            };
            do_op(
                &mut editor,
                &mut active.level,
                EditOp::Place(Element::Signal(SignalDef { cell, at, kind })),
            );
            commands.trigger(crate::audio::SfxKind::BuildingSound);
        }
        Tool::Source if active.sandbox => {
            let dir = station_dir(editor.variant);
            if !can_place_station(&active.level, cell, dir) {
                return;
            }
            let id = SourceId(next_id(active.level.sources.iter().map(|s| s.id.0)));
            do_op(
                &mut editor,
                &mut active.level,
                EditOp::PlaceSource(stellwerk_sim::level::SourceDef {
                    id,
                    cell,
                    dir,
                    label: String::new(),
                }),
            );
            commands.trigger(crate::audio::SfxKind::BuildingSound);
        }
        Tool::Sink if active.sandbox => {
            let dir = station_dir(editor.variant);
            if !can_place_station(&active.level, cell, dir) {
                return;
            }
            let id = SinkId(next_id(active.level.sinks.iter().map(|s| s.id.0)));
            do_op(
                &mut editor,
                &mut active.level,
                EditOp::PlaceSink(stellwerk_sim::level::SinkDef {
                    id,
                    cell,
                    dir,
                    label: format!("Z{}", id.0),
                }),
            );
            commands.trigger(crate::audio::SfxKind::BuildingSound);
        }
        Tool::Source | Tool::Sink => {}
        // Sandbox case returns early above; outside the sandbox the tool is
        // unreachable (its hotkey is sandbox-gated).
        Tool::Block => {}
        Tool::Erase => erase_at(&mut editor, &mut active, cell, cursor),
        Tool::Select => {
            let has_switch = editor.layout.switches.iter().any(|s| s.cell == cell);
            editor.selected_switch = has_switch.then_some(cell);
        }
    }
}

fn next_id(used: impl Iterator<Item = u32>) -> u32 {
    used.max().map_or(0, |m| m + 1)
}

/// Interior cells of the drag path get the piece connecting entry and exit
/// direction; start/end cells stay open (drags begin/end on existing track).
/// Returns `true` when at least one piece was actually placed (so the caller
/// can play the build sound only on a real placement, not an empty/blocked drag).
fn finish_track_drag(editor: &mut Editor, level: &mut Level, merged: &Layout, path: &[Cell]) -> bool {
    let dir_between = |from: Cell, to: Cell| -> Option<Dir8> {
        let delta = (to.x - from.x, to.y - from.y);
        Dir8::ALL.into_iter().find(|d| d.cell_offset() == delta)
    };

    let mut ops = Vec::new();
    let mut placed: Vec<TrackPiece> = Vec::new();
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
        let piece = TrackPiece { cell: cur, a, b };
        // Same gate as click placement, plus connector clashes against
        // pieces from earlier in this very drag.
        let drag_conflict = placed
            .iter()
            .any(|p| p.cell == cur && [p.a, p.b].iter().any(|d| *d == a || *d == b));
        if !can_place_piece(level, merged, &piece) || drag_conflict {
            continue;
        }
        placed.push(piece);
        ops.push(EditOp::Place(Element::Piece(piece)));
    }

    if ops.is_empty() && path.len() == 1 {
        // No drag: click places the current R/T-rotated form.
        let (a, b) = editor.track_form;
        let piece = TrackPiece {
            cell: path[0],
            a,
            b,
        };
        if can_place_piece(level, merged, &piece) {
            do_op(editor, level, EditOp::Place(Element::Piece(piece)));
            return true;
        }
        return false;
    }
    if !ops.is_empty() {
        do_op(editor, level, EditOp::Group(ops));
        return true;
    }
    false
}

/// Applies a Block-tool stroke. The first cell picks the mode for the whole
/// stroke: starting on an empty buildable cell carves blocks, starting on an
/// existing hole restores them. Cells ineligible for the chosen mode are
/// skipped, and the stroke is one undo step. Returns whether anything changed.
fn apply_block_stroke(editor: &mut Editor, level: &mut Level, merged: &Layout, path: &[Cell]) -> bool {
    let Some(&first) = path.first() else {
        return false;
    };
    let carve = can_block_cell(level, merged, first);
    let holes = crate::board::blocked_cells(&level.buildable);
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

/// Removal priority: (sandbox: source/sink at the connector) → signal at the
/// nearest connector → switch → piece. Designer track is untouchable.
fn erase_at(editor: &mut Editor, active: &mut ActiveLevel, cell: Cell, cursor: Vec2) {
    let at = board::nearest_connector(cell, cursor);
    if active.sandbox {
        // A sandbox block holds nothing else (blocking requires the cell empty),
        // so erasing it just restores buildability. Sandbox only: a campaign
        // level's non-buildable cells are its authored shape, not blocks.
        if board::is_blocked(&active.level.buildable, cell) {
            do_op(editor, &mut active.level, EditOp::SetBuildable { cell, on: true });
            return;
        }
        if let Some(source) = active
            .level
            .sources
            .iter()
            .find(|s| s.cell == cell && s.dir == at)
            .cloned()
        {
            do_op(editor, &mut active.level, EditOp::RemoveSource(source));
            return;
        }
        if let Some(sink) = active
            .level
            .sinks
            .iter()
            .find(|s| s.cell == cell && s.dir == at)
            .cloned()
        {
            // Schedule entries pointing at the removed sink would be a
            // permanent validation error — drop them too. Bundle them with the
            // sink into one Group (rows highest-first so earlier removals don't
            // shift the later indices) so a single undo restores all of it.
            let mut ops: Vec<EditOp> = active
                .level
                .schedule
                .iter()
                .enumerate()
                .filter(|(_, e)| e.sink == sink.id)
                .map(|(row, e)| EditOp::ScheduleRemove {
                    row,
                    entry: e.clone(),
                })
                .collect();
            ops.reverse();
            ops.push(EditOp::RemoveSink(sink));
            do_op(editor, &mut active.level, EditOp::Group(ops));
            return;
        }
    }
    if let Some(signal) = editor
        .layout
        .signals
        .iter()
        .find(|s| s.cell == cell && s.at == at)
        .or_else(|| editor.layout.signals.iter().find(|s| s.cell == cell))
        .copied()
    {
        do_op(editor, &mut active.level, EditOp::Remove(Element::Signal(signal)));
        return;
    }
    if let Some(switch) = editor
        .layout
        .switches
        .iter()
        .find(|s| s.cell == cell)
        .cloned()
    {
        if editor.selected_switch == Some(cell) {
            editor.selected_switch = None;
        }
        do_op(editor, &mut active.level, EditOp::Remove(Element::Switch(switch)));
        return;
    }
    if let Some(piece) = editor
        .layout
        .pieces
        .iter()
        .find(|p| p.cell == cell)
        .copied()
    {
        do_op(editor, &mut active.level, EditOp::Remove(Element::Piece(piece)));
    }
}
