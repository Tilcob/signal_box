//! Click-to-jump on located lines (camera recentre + click sound) and the
//! hover/press highlight that marks them as clickable.

use bevy::prelude::*;

use super::{RowJump, ScrollbarDrag};

/// Row highlight while hovering / pressing a clickable (located) line.
const ROW_HOVER: Color = Color::srgba(1.0, 1.0, 1.0, 0.08);
const ROW_PRESSED: Color = Color::srgba(1.0, 1.0, 1.0, 0.16);

/// Click on a located console line → recentre the camera there + click sound.
/// Suppressed while dragging the scrollbar (the drag can sweep the pointer over
/// rows, which must not fire jumps).
pub(super) fn console_jump_clicks(
    rows: Query<(&Interaction, &RowJump), Changed<Interaction>>,
    drag: Res<ScrollbarDrag>,
    mut cameras: Query<&mut Transform, With<crate::camera::MainCamera>>,
    mut commands: Commands,
) {
    if drag.0 {
        return;
    }
    for (interaction, jump) in &rows {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let Some(pos) = jump.0 else { continue };
        commands.trigger(crate::audio::SfxKind::ButtonClick);
        if let Ok(mut transform) = cameras.single_mut() {
            transform.translation.x = pos.x;
            transform.translation.y = pos.y;
        }
    }
}

/// Hover/press highlight on clickable (located) rows only; plain rows stay clear.
pub(super) fn console_row_feedback(mut rows: Query<(&Interaction, &RowJump, &mut BackgroundColor)>) {
    for (interaction, jump, mut bg) in &mut rows {
        let target = if jump.0.is_some() {
            match interaction {
                Interaction::Pressed => ROW_PRESSED,
                Interaction::Hovered => ROW_HOVER,
                Interaction::None => Color::NONE,
            }
        } else {
            Color::NONE
        };
        if bg.0 != target {
            bg.0 = target;
        }
    }
}
