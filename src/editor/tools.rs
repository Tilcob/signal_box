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
    EraseTarget, Placement, auto_station_orientation, auto_switch_orientation, can_block_cell,
    can_place_signal, can_place_station, erase_target, plan_piece, plan_switch, signal_stub,
    station_dir, switch_variants,
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
            if finish_track_drag(&mut editor, &mut active.level, &path) {
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

    // Erase: a held drag wipes every cell the cursor crosses; a plain click is
    // a one-cell stroke. Like Track and Block it collects a path and applies it
    // as one undo step.
    if editor.tool == Tool::Erase {
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
            apply_erase_stroke(&mut editor, &mut active, &path, cursor);
        }
        return;
    }

    if !buttons.just_pressed(MouseButton::Left) || over_ui {
        return;
    }

    match editor.tool {
        Tool::Track => unreachable!("handled above"),
        Tool::Switch => {
            let plan = plan_switch(&active.level, &editor.layout, cell);
            if matches!(plan, Placement::Blocked) {
                return;
            }
            // Auto-orient from the surrounding track when the junction admits
            // exactly one legal stem (the player needn't aim it); otherwise the
            // R/T-cycled preset.
            let (stem, branches) = auto_switch_orientation(&active.level, merged, cell).unwrap_or_else(|| {
                let variants = switch_variants();
                variants[editor.variant.rem_euclid(variants.len() as i32) as usize]
            });
            let switch = Element::Switch(SwitchDef {
                cell,
                stem,
                branches,
                default_branch: 0,
                rules: vec![],
            });
            place_replacing(&mut editor, &mut active.level, plan, switch);
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
                EditOp::Place(Element::Signal(SignalDef {
                    cell,
                    at,
                    kind,
                    priority: 0,
                })),
            );
            commands.trigger(crate::audio::SfxKind::BuildingSound);
        }
        Tool::Source if active.sandbox => {
            // Snap outward at a level edge; R/T drives it where ambiguous.
            let dir = auto_station_orientation(&active.level, cell)
                .unwrap_or_else(|| station_dir(editor.variant));
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
            let dir = auto_station_orientation(&active.level, cell)
                .unwrap_or_else(|| station_dir(editor.variant));
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
        Tool::Erase => unreachable!("handled above"),
        Tool::Select => {
            // Switch takes precedence (a switch cell holds no signal); otherwise
            // pick the signal at the connector nearest the cursor, then any on
            // the cell. Selecting one clears the other — the panels are mutually
            // exclusive.
            if editor.layout.switches.iter().any(|s| s.cell == cell) {
                editor.selected_switch = Some(cell);
                editor.selected_signal = None;
            } else {
                let at = board::nearest_connector(cell, cursor);
                editor.selected_signal = editor
                    .layout
                    .signals
                    .iter()
                    .find(|s| s.cell == cell && s.at == at)
                    .or_else(|| editor.layout.signals.iter().find(|s| s.cell == cell))
                    .map(|s| (s.cell, s.at));
                editor.selected_switch = None;
            }
        }
    }
}

fn next_id(used: impl Iterator<Item = u32>) -> u32 {
    used.max().map_or(0, |m| m + 1)
}

/// Interior cells of the drag path get the piece connecting entry and exit
/// direction; the two endpoints (each knows only one drag direction) get a
/// straight piece along it, so a free draw is continuous from the very first
/// cell. An endpoint that sits on existing track (a source/sink anchor) makes
/// the straight clash, so it is skipped and the cell is left to its existing
/// piece — the anchor→anchor campaign drag is unchanged. Returns `true` when at
/// least one piece was actually placed (so the caller plays the build sound
/// only on a real placement, not an empty/blocked drag).
fn finish_track_drag(editor: &mut Editor, level: &mut Level, path: &[Cell]) -> bool {
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

/// Commits a placement plan: a plain `Place`, or a `Replace` that first removes
/// the player elements giving way — clearing any selection pointing at them —
/// and places the new element in the SAME undo step. `Blocked` is filtered by
/// the caller and is a no-op here.
fn place_replacing(editor: &mut Editor, level: &mut Level, plan: Placement, new: Element) {
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
fn apply_erase_stroke(editor: &mut Editor, active: &mut ActiveLevel, path: &[Cell], cursor: Vec2) {
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

#[cfg(test)]
mod tests {
    use super::*;
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
