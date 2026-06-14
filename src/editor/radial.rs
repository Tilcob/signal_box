//! Radial track menu: right-click a cell with the Track tool to pop eight
//! boxes around it — one per compass direction. Each box offers a track form
//! exiting toward that direction (the entry connector stays fixed); illegal
//! exits (kinks, the entry itself) are shown greyed. Picking a box sets the
//! ghost form; R/T then rotate it. This is the spatial counterpart to R/T:
//! rotation walks the orientations, the menu jumps to a secondary form.

use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use stellwerk_sim::grid::{Dir8, pair_len};

use crate::board::{self, CELL};
use crate::camera::{MainCamera, cursor_world};
use crate::state::{Editor, Tool};

/// Half-edge of a menu box, in world units.
const BOX_HALF: f32 = CELL * 0.16;

/// Opens (RMB), draws and resolves (LMB / Esc) the radial track menu. Runs
/// after `pointer` and `leave_to_select` so those see the menu still open on
/// the click/Esc frame and yield to it.
pub(super) fn radial_menu(
    mut gizmos: Gizmos,
    buttons: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    cameras: Query<(&Camera, &GlobalTransform), With<MainCamera>>,
    ui: Query<&Interaction>,
    mut editor: ResMut<Editor>,
) {
    let over_ui = ui.iter().any(|i| *i != Interaction::None);
    let cursor = cursor_world(&windows, &cameras);

    // Open / close on right-click (only meaningful for the Track tool).
    if buttons.just_pressed(MouseButton::Right) && !over_ui && editor.tool == Tool::Track {
        let bypass = editor.bypass_change_detection();
        bypass.radial = match (bypass.radial, cursor) {
            (None, Some(c)) => Some(board::world_cell(c)),
            _ => None, // a second right-click closes
        };
        return;
    }

    let Some(menu_cell) = editor.radial else {
        return;
    };
    // Esc closes the menu (leave_to_select yields while it is open).
    if keys.just_pressed(KeyCode::Escape) {
        editor.bypass_change_detection().radial = None;
        return;
    }

    let entry = editor.track_form.0;
    let center = board::cell_world(menu_cell);
    let clicked = buttons.just_pressed(MouseButton::Left) && !over_ui;
    let mut hit_a_box = false;

    for dir in Dir8::ALL {
        let legal = dir != entry && pair_len(entry, dir).is_some();
        let box_center = board::connector_world(menu_cell, dir);
        let hovered = cursor.is_some_and(|c| {
            (c.x - box_center.x).abs() <= BOX_HALF && (c.y - box_center.y).abs() <= BOX_HALF
        });

        let color = if !legal {
            Color::srgba(0.4, 0.4, 0.45, 0.5)
        } else if hovered {
            Color::srgba(0.6, 1.0, 0.8, 0.95)
        } else {
            Color::srgba(0.5, 0.8, 1.0, 0.8)
        };
        gizmos.rect_2d(
            Isometry2d::from_translation(box_center),
            Vec2::splat(BOX_HALF * 2.0),
            color,
        );
        if legal {
            // Miniature of the resulting form, oriented like it would sit in
            // the cell: two stubs from the box centre toward entry and exit.
            for d in [entry, dir] {
                let toward = (board::connector_world(menu_cell, d) - center).normalize_or_zero();
                gizmos.line_2d(box_center, box_center + toward * (BOX_HALF * 0.8), color);
            }
        }

        if clicked && legal && hovered {
            editor.bypass_change_detection().track_form = (entry, dir);
            editor.bypass_change_detection().radial = None;
            hit_a_box = true;
        }
    }

    // A left-click that missed every box closes the menu without choosing.
    if clicked && !hit_a_box {
        editor.bypass_change_detection().radial = None;
    }
}
