//! Tool input: hotkeys (incl. layout-aware undo/redo), the board pointer
//! with placement/erase, track drags and leaving back to the level select.

use bevy::input::keyboard::Key;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use stellwerk_sim::grid::{Cell, Dir8, pair_len};
use stellwerk_sim::layout::{Layout, SignalDef, SignalKind, SwitchDef, TrackPiece};
use stellwerk_sim::level::Level;
use stellwerk_sim::units::{SinkId, SourceId};

use super::ops::{EditOp, Element, apply, do_op, invert};
use super::placement::{
    can_place_piece, can_place_signal, can_place_station, can_place_switch, signal_stub,
    switch_variants,
};
use crate::board;
use crate::camera::{MainCamera, cursor_world};
use crate::levels::{Progress, save_sandbox};
use crate::state::{ActiveLevel, Editor, GameState, Tool};

pub(super) fn hotkeys(
    keys: Res<ButtonInput<KeyCode>>,
    logical: Res<ButtonInput<Key>>,
    active: Option<Res<ActiveLevel>>,
    mut editor: ResMut<Editor>,
) {
    let sandbox = active.is_some_and(|a| a.sandbox);
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
    let undo = logical.just_pressed(chr("z")) || logical.just_pressed(chr("Z"));
    let redo = logical.just_pressed(chr("y")) || logical.just_pressed(chr("Y"));
    if ctrl && undo {
        let editor = &mut *editor; // re-borrow WITH change detection
        if let Some(op) = editor.undo.pop() {
            let inverse = invert(&op);
            apply(&mut editor.layout, &inverse);
            editor.redo.push(op);
        }
    }
    if ctrl && redo {
        let editor = &mut *editor;
        if let Some(op) = editor.redo.pop() {
            apply(&mut editor.layout, &op);
            editor.undo.push(op);
        }
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
    let merged = active.level.fixed.merged(&editor.layout);

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
            finish_track_drag(&mut editor, &active.level, &merged, &path);
        }
        return;
    }

    if !buttons.just_pressed(MouseButton::Left) || over_ui {
        return;
    }

    match editor.tool {
        Tool::Track => unreachable!("handled above"),
        Tool::Switch => {
            if !can_place_switch(&active.level, &merged, cell) {
                return;
            }
            let variants = switch_variants();
            let (stem, branches) =
                variants[editor.variant.rem_euclid(variants.len() as i32) as usize];
            do_op(
                &mut editor,
                EditOp::Place(Element::Switch(SwitchDef {
                    cell,
                    stem,
                    branches,
                    default_branch: 0,
                    rules: vec![],
                })),
            );
            commands.trigger(crate::audio::SfxKind::Switch);
        }
        Tool::SignalBlock | Tool::SignalChain => {
            // Direction is keyboard-driven (R/T cycle the cell's stubs), not
            // taken from the cursor angle.
            let Some(at) = signal_stub(&merged, cell, editor.variant) else {
                return;
            };
            if !can_place_signal(&active.level, &merged, cell, at) {
                return;
            }
            let kind = if editor.tool == Tool::SignalBlock {
                SignalKind::Block
            } else {
                SignalKind::Chain
            };
            do_op(
                &mut editor,
                EditOp::Place(Element::Signal(SignalDef { cell, at, kind })),
            );
        }
        Tool::Source if active.sandbox => {
            let dir = board::nearest_connector(cell, cursor);
            if !can_place_station(&active.level, cell, dir) {
                return;
            }
            let id = SourceId(next_id(active.level.sources.iter().map(|s| s.id.0)));
            active
                .level
                .sources
                .push(stellwerk_sim::level::SourceDef { id, cell, dir });
        }
        Tool::Sink if active.sandbox => {
            let dir = board::nearest_connector(cell, cursor);
            if !can_place_station(&active.level, cell, dir) {
                return;
            }
            let id = SinkId(next_id(active.level.sinks.iter().map(|s| s.id.0)));
            active.level.sinks.push(stellwerk_sim::level::SinkDef {
                id,
                cell,
                dir,
                label: format!("Z{}", id.0),
            });
        }
        Tool::Source | Tool::Sink => {}
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
fn finish_track_drag(editor: &mut Editor, level: &Level, merged: &Layout, path: &[Cell]) {
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
            do_op(editor, EditOp::Place(Element::Piece(piece)));
        }
        return;
    }
    if !ops.is_empty() {
        do_op(editor, EditOp::Group(ops));
    }
}

/// Removal priority: (sandbox: source/sink at the connector) → signal at the
/// nearest connector → switch → piece. Designer track is untouchable.
fn erase_at(editor: &mut Editor, active: &mut ActiveLevel, cell: Cell, cursor: Vec2) {
    let at = board::nearest_connector(cell, cursor);
    if active.sandbox {
        let sources_before = active.level.sources.len();
        active
            .level
            .sources
            .retain(|s| !(s.cell == cell && s.dir == at));
        if active.level.sources.len() != sources_before {
            return;
        }
        let sinks_before = active.level.sinks.len();
        let removed: Vec<SinkId> = active
            .level
            .sinks
            .iter()
            .filter(|s| s.cell == cell && s.dir == at)
            .map(|s| s.id)
            .collect();
        active
            .level
            .sinks
            .retain(|s| !(s.cell == cell && s.dir == at));
        if active.level.sinks.len() != sinks_before {
            // Schedule entries pointing at a removed sink would be a
            // permanent validation error — drop them along.
            active.level.schedule.retain(|e| !removed.contains(&e.sink));
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
        do_op(editor, EditOp::Remove(Element::Signal(signal)));
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
        do_op(editor, EditOp::Remove(Element::Switch(switch)));
        return;
    }
    if let Some(piece) = editor
        .layout
        .pieces
        .iter()
        .find(|p| p.cell == cell)
        .copied()
    {
        do_op(editor, EditOp::Remove(Element::Piece(piece)));
    }
}

/// Esc returns to level select (build and sandbox level are autosaved).
pub(super) fn leave_to_select(
    keys: Res<ButtonInput<KeyCode>>,
    active: Option<Res<ActiveLevel>>,
    editor: Res<Editor>,
    mut progress: ResMut<Progress>,
    mut next: ResMut<NextState<GameState>>,
) {
    if !keys.just_pressed(KeyCode::Escape) {
        return;
    }
    // Esc first closes an open radial menu (handled by `radial_menu`, which
    // runs after this); only an Esc with no menu open leaves the level.
    if editor.radial.is_some() {
        return;
    }
    if let Some(active) = active {
        progress.entry(&active.id).layout = editor.layout.clone();
        progress.save();
        if active.sandbox {
            save_sandbox(&active.level);
        }
    }
    next.set(GameState::LevelSelect);
}
