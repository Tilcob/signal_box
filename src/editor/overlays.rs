//! Hover highlight, ghost previews (red when placement is blocked) and
//! validation error markers via gizmos.

use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use stellwerk_sim::ValidationError;
use stellwerk_sim::grid::{Cell, Dir8, pair_len};
use stellwerk_sim::layout::TrackPiece;

use super::Diagnostics;
use super::placement::{
    EraseTarget, Placement, auto_station_orientation, can_block_cell, can_place_signal,
    can_place_station, erase_target, plan_piece, plan_switch, signal_stub, station_dir,
    switch_variants,
};
use crate::board::{self, CELL};
use crate::camera::{MainCamera, cursor_world};
use crate::state::{ActiveLevel, Editor, Tool};

pub(super) fn draw_overlays(
    mut gizmos: Gizmos,
    windows: Query<&Window, With<PrimaryWindow>>,
    cameras: Query<(&Camera, &GlobalTransform), With<MainCamera>>,
    active: Option<Res<ActiveLevel>>,
    editor: Res<Editor>,
    merged: Res<super::MergedLayout>,
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
        let merged = &merged.0;
        let blocked = Color::srgba(1.0, 0.35, 0.3, 0.6);
        match editor.tool {
            Tool::Track => {
                let (a, b) = editor.track_form;
                // Building over the player's own clashing track is allowed (it
                // replaces it), so only a hard block (off-board / fixed) reds out.
                let ok = match &active {
                    Some(active) => !matches!(
                        plan_piece(&active.level, &editor.layout, &TrackPiece { cell, a, b }),
                        Placement::Blocked
                    ),
                    None => true,
                };
                // The selected form glows yellow so it reads clearly against the
                // faint blue ring of alternatives below; red only on a hard block.
                let ghost = if ok {
                    Color::srgba(1.0, 0.85, 0.2, 0.9)
                } else {
                    blocked
                };
                gizmos.line_2d(board::connector_world(cell, a), center, ghost);
                gizmos.line_2d(board::connector_world(cell, b), center, ghost);
                // Curve ring: a faint fan to every OTHER legal exit from the
                // entry `a`, so the available curve forms are always visible
                // (Ctrl+wheel cycles through them). Only the alternatives — the
                // current form is the bright ghost above. Hidden mid-drag, where
                // the free-draw owns the board.
                if editor.drag.is_none() {
                    let hint = Color::srgba(0.7, 0.8, 1.0, 0.22);
                    for d in Dir8::ALL {
                        if d != a && d != b && pair_len(a, d).is_some() {
                            let end = board::connector_world(cell, d);
                            gizmos.line_2d(center, end, hint);
                            gizmos.circle_2d(Isometry2d::from_translation(end), 3.0, hint);
                        }
                    }
                }
            }
            Tool::Switch => {
                let variants = switch_variants();
                let (stem, branches) =
                    variants[editor.variant.rem_euclid(variants.len() as i32) as usize];
                let ok = match &active {
                    Some(active) => {
                        !matches!(plan_switch(&active.level, &editor.layout, cell), Placement::Blocked)
                    }
                    None => true,
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
                // R/T pick the gated connector among the cell's stubs.
                if let Some(at) = signal_stub(merged, cell, editor.variant) {
                    let connector = board::connector_world(cell, at);
                    let ok = match &active {
                        Some(active) => can_place_signal(&active.level, merged, cell, at),
                        None => true,
                    };
                    let ghost = if ok {
                        Color::srgba(0.4, 1.0, 0.6, 0.6)
                    } else {
                        blocked
                    };
                    gizmos.circle_2d(Isometry2d::from_translation(connector), 10.0, ghost);
                    // Gated travel direction (out of the cell across `at`),
                    // capped with an arrowhead so the direction reads at a
                    // glance (mirrors the placed signal's arrow) — shown before
                    // placing, so a backwards signal is no surprise.
                    let outward = (connector - center).normalize_or_zero();
                    let tip = connector + outward * 26.0;
                    gizmos.line_2d(connector, tip, ghost);
                    let perp = outward.perp();
                    let back = tip - outward * 9.0;
                    gizmos.line_2d(tip, back + perp * 7.0, ghost);
                    gizmos.line_2d(tip, back - perp * 7.0, ghost);
                }
            }
            Tool::Block => {
                // Highlight the target cell: ok if it can be blocked or is an
                // existing hole (click toggles), blocked-red otherwise.
                let ok = match &active {
                    Some(active) => {
                        can_block_cell(&active.level, merged, cell)
                            || board::is_blocked(&active.level.buildable, cell)
                    }
                    None => false,
                };
                let ghost = if ok {
                    Color::srgba(0.5, 0.5, 0.55, 0.6)
                } else {
                    blocked
                };
                gizmos.rect_2d(Isometry2d::from_translation(center), Vec2::splat(CELL - 8.0), ghost);
            }
            Tool::Source | Tool::Sink => {
                // Mirror placement: snap outward at a level edge, else the
                // R/T-cycled connector. Showing the real direction here is what
                // makes the auto-orientation visible before the click.
                let at = active
                    .as_ref()
                    .and_then(|a| auto_station_orientation(&a.level, cell))
                    .unwrap_or_else(|| station_dir(editor.variant));
                let connector = board::connector_world(cell, at);
                let ok = active
                    .as_ref()
                    .is_none_or(|a| can_place_station(&a.level, cell, at));
                let ghost = if ok {
                    Color::srgba(0.4, 1.0, 0.6, 0.6)
                } else {
                    blocked
                };
                // Ghost of the model that will be placed (matches `draw_stations`):
                // a source's inward chevrons, a sink's buffer-stop bar.
                let outward = (connector - center).normalize_or_zero();
                if editor.tool == Tool::Source {
                    gizmos.line_2d(connector, connector + outward * 26.0, ghost);
                    let inward = -outward;
                    let perp = inward.perp();
                    for i in 0..2 {
                        let tip = connector - outward * (2.0 + i as f32 * 10.0);
                        let back = tip - inward * 9.0;
                        gizmos.line_2d(tip, back + perp * 7.0, ghost);
                        gizmos.line_2d(tip, back - perp * 7.0, ghost);
                    }
                } else {
                    let end = connector + outward * 22.0;
                    let perp = outward.perp();
                    gizmos.line_2d(connector, end, ghost);
                    gizmos.line_2d(end - perp * 13.0, end + perp * 13.0, ghost);
                }
            }
            Tool::Erase => {
                // Outline the element the next click/drag would erase, so it is
                // clear WHAT is about to go. Connector picked from the cursor
                // (like the click) so the right signal is targeted on a busy cell.
                if let Some(active) = &active {
                    let at = board::nearest_connector(cell, cursor);
                    if let Some(target) =
                        erase_target(&editor.layout, &active.level, active.sandbox, cell, at)
                    {
                        draw_erase_outline(&mut gizmos, &target);
                    }
                }
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

/// Red highlight of the element the erase tool is hovering. Track/switch get
/// their bands re-stroked plus a cell box; a signal gets a ring at its precise
/// connector (a cell box would wrongly imply the whole cell goes); blocks and
/// stations get the cell box.
fn draw_erase_outline(gizmos: &mut Gizmos, target: &EraseTarget) {
    let red = Color::srgba(1.0, 0.2, 0.2, 0.9);
    match target {
        EraseTarget::Piece(p) => {
            let center = board::cell_world(p.cell);
            gizmos.line_2d(board::connector_world(p.cell, p.a), center, red);
            gizmos.line_2d(board::connector_world(p.cell, p.b), center, red);
            red_cell(gizmos, p.cell, red);
        }
        EraseTarget::Switch(s) => {
            let center = board::cell_world(s.cell);
            gizmos.line_2d(board::connector_world(s.cell, s.stem), center, red);
            for b in s.branches {
                gizmos.line_2d(board::connector_world(s.cell, b), center, red);
            }
            red_cell(gizmos, s.cell, red);
        }
        EraseTarget::Signal(s) => {
            let connector = board::connector_world(s.cell, s.at);
            gizmos.circle_2d(Isometry2d::from_translation(connector), 12.0, red);
        }
        EraseTarget::Block(cell) => red_cell(gizmos, *cell, red),
        EraseTarget::Source(s) => red_cell(gizmos, s.cell, red),
        EraseTarget::Sink(s) => red_cell(gizmos, s.cell, red),
    }
}

fn red_cell(gizmos: &mut Gizmos, cell: Cell, color: Color) {
    gizmos.rect_2d(
        Isometry2d::from_translation(board::cell_world(cell)),
        Vec2::splat(CELL - 2.0),
        color,
    );
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
