//! Board pointer dispatch: routes the left-button click/drag to the active
//! tool. Track/Block/Erase collect a cell path and delegate to their stroke
//! module; Switch/Signal/Source/Sink place on a single click; Select opens a
//! config panel.

use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use stellwerk_sim::layout::{SignalDef, SignalKind, SwitchDef};
use stellwerk_sim::units::{PlatformId, SinkId, SourceId};

use super::commit::place_replacing;
use super::strokes::{apply_block_stroke, apply_erase_stroke};
use super::track::finish_track_drag;
use crate::board;
use crate::camera::{MainCamera, cursor_world};
use crate::editor::MergedLayout;
use crate::editor::ops::{EditOp, Element, do_op};
use crate::editor::placement::{
    Placement, auto_station_orientation, auto_switch_orientation, can_place_platform,
    can_place_signal, can_place_station, plan_switch, signal_stub, station_dir, switch_variants,
};
use crate::state::{ActiveLevel, Editor, Tool};

#[allow(clippy::too_many_arguments)]
pub(crate) fn pointer(
    buttons: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    cameras: Query<(&Camera, &GlobalTransform), With<MainCamera>>,
    ui: Query<&Interaction>,
    active: Option<ResMut<ActiveLevel>>,
    mut editor: ResMut<Editor>,
    merged: Res<MergedLayout>,
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
        Tool::Platform if active.sandbox => {
            // A platform usually sits mid-line (interior), where
            // `auto_station_orientation` declines — the through-direction then
            // comes from the R/T-cycled `station_dir`.
            let dir = auto_station_orientation(&active.level, cell)
                .unwrap_or_else(|| station_dir(editor.variant));
            if !can_place_platform(&active.level, cell, dir) {
                return;
            }
            let id = PlatformId(next_id(active.level.platforms.iter().map(|p| p.id.0)));
            do_op(
                &mut editor,
                &mut active.level,
                EditOp::PlacePlatform(stellwerk_sim::level::PlatformDef {
                    id,
                    cell,
                    dir,
                    label: format!("B{}", id.0),
                }),
            );
            commands.trigger(crate::audio::SfxKind::BuildingSound);
        }
        Tool::Source | Tool::Sink | Tool::Platform => {}
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
