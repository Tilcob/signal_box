//! Edit mode: tools, drag track drawing, invertible edit ops with undo/redo,
//! live validation + reachability warnings (plan M1 §2/§3; M2: sandbox
//! source/sink tools).
//!
//! Validation is never modal: faulty elements glow on the board, the start
//! switch stays locked while errors exist. Reachability problems are
//! warnings only — watching the misrouting happen is a lesson (Säule 4).
//!
//! Sandbox note (M2-minimal): source/sink placement and the schedule editor
//! mutate the LEVEL, not the layout — they sit outside the undo stack.

use bevy::input::keyboard::Key;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use stellwerk_sim::grid::{Cell, Dir8, pair_len};
use stellwerk_sim::layout::{Layout, SignalDef, SignalKind, SwitchDef, TrackPiece};
use stellwerk_sim::level::{Level, SinkDef, SourceDef};
use stellwerk_sim::units::{SinkId, SourceId};
use stellwerk_sim::{ValidationError, check_reachability, validate};

use crate::board::{self, CELL};
use crate::camera::{MainCamera, cursor_world};
use crate::levels::{Progress, save_sandbox};
use crate::state::{ActiveLevel, Diagnostics, Editor, GameState, Tool};

// --- Edit operations ---------------------------------------------------------

#[derive(Debug, Clone)]
pub enum Element {
    Piece(TrackPiece),
    Switch(SwitchDef),
    Signal(SignalDef),
}

/// Invertible build action — the undo/redo vocabulary, and in M2 the basis
/// of the sharing format.
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

fn apply(layout: &mut Layout, op: &EditOp) {
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
        EditOp::Group(ops) => EditOp::Group(ops.iter().rev().map(invert).collect()),
    }
}

/// Applies an op and records it for undo. Public for the switch panel (ui).
pub fn do_op(editor: &mut Editor, op: EditOp) {
    apply(&mut editor.layout, &op);
    editor.undo.push(op);
    editor.redo.clear();
}

// --- Placement variants -------------------------------------------------------

/// All 16 legal connector pairs, R-cycled for click placement.
fn piece_variants() -> Vec<(Dir8, Dir8)> {
    let mut out = Vec::new();
    for a in Dir8::ALL {
        for b in Dir8::ALL {
            if a.index() < b.index() && pair_len(a, b).is_some() {
                out.push((a, b));
            }
        }
    }
    out
}

/// 8 switch presets: cardinal stem, branches = straight-through + 45° turn.
fn switch_variants() -> Vec<(Dir8, [Dir8; 2])> {
    let rot = |d: Dir8, k: usize| Dir8::ALL[(d.index() as usize + k) % 8];
    let mut out = Vec::new();
    for stem in [Dir8::W, Dir8::E, Dir8::N, Dir8::S] {
        let straight = stem.opposite();
        out.push((stem, [straight, rot(straight, 1)]));
        out.push((stem, [straight, rot(straight, 7)]));
    }
    out
}

// --- Placement gates ----------------------------------------------------------
//
// Hard placement rules (occupied/off-board) are rejected at the tool instead
// of being placed and flagged: stacking a switch on a switch is never a
// puzzle state worth inspecting. Cross-cell problems (junction without
// switch, reachability) stay non-modal — they glow as diagnostics.

/// Buildable cell, no switch there, and both connectors still free —
/// crossings with disjoint connectors stay legal.
fn can_place_piece(level: &Level, merged: &Layout, piece: &TrackPiece) -> bool {
    level.buildable.contains(&piece.cell)
        && !merged.switches.iter().any(|s| s.cell == piece.cell)
        && !merged.pieces.iter().any(|p| {
            p.cell == piece.cell && [p.a, p.b].iter().any(|d| *d == piece.a || *d == piece.b)
        })
}

/// Switch cells are exclusive: buildable and completely empty.
fn can_place_switch(level: &Level, merged: &Layout, cell: Cell) -> bool {
    level.buildable.contains(&cell)
        && !merged.pieces.iter().any(|p| p.cell == cell)
        && !merged.switches.iter().any(|s| s.cell == cell)
}

/// Signals need track under their connector and may not stack.
fn can_place_signal(level: &Level, merged: &Layout, cell: Cell, at: Dir8) -> bool {
    level.buildable.contains(&cell)
        && merged.has_stub(cell, at)
        && !merged.signals.iter().any(|s| s.cell == cell && s.at == at)
}

pub struct EditorPlugin;

impl Plugin for EditorPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (hotkeys, pointer, draw_overlays, revalidate, leave_to_select)
                .chain()
                .run_if(in_state(GameState::Edit)),
        );
    }
}

fn hotkeys(
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
    if keys.just_pressed(KeyCode::Digit5) || keys.just_pressed(KeyCode::KeyX) {
        bypass.tool = Tool::Erase;
    }
    if sandbox && keys.just_pressed(KeyCode::Digit6) {
        bypass.tool = Tool::Source;
    }
    if sandbox && keys.just_pressed(KeyCode::Digit7) {
        bypass.tool = Tool::Sink;
    }
    if keys.just_pressed(KeyCode::KeyR) {
        bypass.variant = bypass.variant.wrapping_add(1);
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
fn pointer(
    buttons: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    cameras: Query<(&Camera, &GlobalTransform), With<MainCamera>>,
    ui: Query<&Interaction>,
    active: Option<ResMut<ActiveLevel>>,
    mut editor: ResMut<Editor>,
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
            let (stem, branches) = variants[editor.variant % variants.len()];
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
        }
        Tool::SignalBlock | Tool::SignalChain => {
            let at = board::nearest_connector(cell, cursor);
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
            let id = SourceId(next_id(active.level.sources.iter().map(|s| s.id.0)));
            active.level.sources.push(SourceDef { id, cell, dir });
        }
        Tool::Sink if active.sandbox => {
            let dir = board::nearest_connector(cell, cursor);
            let id = SinkId(next_id(active.level.sinks.iter().map(|s| s.id.0)));
            active.level.sinks.push(SinkDef {
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
        // No drag: click places the R-cycled variant.
        let variants = piece_variants();
        let (a, b) = variants[editor.variant % variants.len()];
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

/// Hover highlight, ghost preview (red when placement is blocked) and error
/// markers via gizmos.
fn draw_overlays(
    mut gizmos: Gizmos,
    windows: Query<&Window, With<PrimaryWindow>>,
    cameras: Query<(&Camera, &GlobalTransform), With<MainCamera>>,
    active: Option<Res<ActiveLevel>>,
    editor: Res<Editor>,
    diagnostics: Res<Diagnostics>,
) {
    if let Some(cursor) = cursor_world(&windows, &cameras) {
        let cell = board::world_cell(cursor);
        let center = board::cell_world(cell);
        gizmos.rect_2d(
            Isometry2d::from_translation(center),
            Vec2::splat(CELL - 4.0),
            Color::srgba(0.6, 0.7, 0.9, 0.35),
        );
        let merged = active
            .as_ref()
            .map(|a| a.level.fixed.merged(&editor.layout));
        let blocked = Color::srgba(1.0, 0.35, 0.3, 0.6);
        match editor.tool {
            Tool::Track => {
                let variants = piece_variants();
                let (a, b) = variants[editor.variant % variants.len()];
                let ok = match (&active, &merged) {
                    (Some(active), Some(merged)) => {
                        can_place_piece(&active.level, merged, &TrackPiece { cell, a, b })
                    }
                    _ => true,
                };
                let ghost = if ok {
                    Color::srgba(0.7, 0.8, 1.0, 0.5)
                } else {
                    blocked
                };
                gizmos.line_2d(board::connector_world(cell, a), center, ghost);
                gizmos.line_2d(board::connector_world(cell, b), center, ghost);
            }
            Tool::Switch => {
                let variants = switch_variants();
                let (stem, branches) = variants[editor.variant % variants.len()];
                let ok = match (&active, &merged) {
                    (Some(active), Some(merged)) => can_place_switch(&active.level, merged, cell),
                    _ => true,
                };
                let ghost = if ok {
                    Color::srgba(1.0, 0.9, 0.4, 0.5)
                } else {
                    blocked
                };
                gizmos.line_2d(board::connector_world(cell, stem), center, ghost);
                for b in branches {
                    gizmos.line_2d(board::connector_world(cell, b), center, ghost);
                }
            }
            Tool::SignalBlock | Tool::SignalChain => {
                let at = board::nearest_connector(cell, cursor);
                let connector = board::connector_world(cell, at);
                let ok = match (&active, &merged) {
                    (Some(active), Some(merged)) => {
                        can_place_signal(&active.level, merged, cell, at)
                    }
                    _ => true,
                };
                let ghost = if ok {
                    Color::srgba(0.4, 1.0, 0.6, 0.6)
                } else {
                    blocked
                };
                gizmos.circle_2d(Isometry2d::from_translation(connector), 10.0, ghost);
                // Gated travel direction (out of the cell across `at`) —
                // shown before placing, so a backwards signal is no surprise.
                let outward = (connector - center).normalize_or_zero();
                gizmos.line_2d(connector, connector + outward * 26.0, ghost);
            }
            Tool::Source | Tool::Sink => {
                let at = board::nearest_connector(cell, cursor);
                gizmos.circle_2d(
                    Isometry2d::from_translation(board::connector_world(cell, at)),
                    10.0,
                    Color::srgba(0.4, 1.0, 0.6, 0.6),
                );
            }
            _ => {}
        }
        if let Some(path) = &editor.drag {
            for pair in path.windows(2) {
                gizmos.line_2d(
                    board::cell_world(pair[0]),
                    board::cell_world(pair[1]),
                    Color::srgba(0.7, 0.8, 1.0, 0.7),
                );
            }
        }
    }

    // Error markers: faulty cells get a red ring (color + shape).
    for error in &diagnostics.errors {
        if let Some(pos) = error_pos(error) {
            gizmos.circle_2d(
                Isometry2d::from_translation(pos),
                CELL * 0.42,
                Color::srgb(1.0, 0.2, 0.2),
            );
        }
    }
}

fn error_pos(error: &ValidationError) -> Option<Vec2> {
    use ValidationError::*;
    let cell = match error {
        IllegalPiecePair { cell, .. }
        | DuplicatePiece { cell, .. }
        | SwitchConnectorClash { cell }
        | SwitchBranchAngle { cell, .. }
        | SwitchDefaultOutOfRange { cell }
        | SwitchRuleBranchOutOfRange { cell }
        | SwitchRuleUnknownSink { cell, .. }
        | SwitchCellNotExclusive { cell }
        | DuplicateSwitch { cell }
        | SignalOffTrack { cell, .. }
        | DuplicateSignal { cell, .. }
        | ConnectorReused { cell, .. }
        | OutsideBuildable { cell } => *cell,
        JunctionWithoutSwitch { point } => return Some(board::point_world(*point)),
        _ => return None,
    };
    Some(board::cell_world(cell))
}

fn revalidate(
    active: Option<Res<ActiveLevel>>,
    editor: Res<Editor>,
    mut diagnostics: ResMut<Diagnostics>,
) {
    let Some(active) = active else { return };
    if !editor.is_changed() && !active.is_changed() {
        return;
    }
    diagnostics.errors = validate(&active.level, &editor.layout);
    diagnostics.unreachable = if diagnostics.errors.is_empty() {
        check_reachability(&active.level, &editor.layout).unwrap_or_default()
    } else {
        Vec::new()
    };
}

/// Esc returns to level select (build and sandbox level are autosaved).
fn leave_to_select(
    keys: Res<ButtonInput<KeyCode>>,
    active: Option<Res<ActiveLevel>>,
    editor: Res<Editor>,
    mut progress: ResMut<Progress>,
    mut next: ResMut<NextState<GameState>>,
) {
    if !keys.just_pressed(KeyCode::Escape) {
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
