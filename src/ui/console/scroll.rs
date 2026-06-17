//! Console scrolling: the hover gate (drives `ConsoleHovered`, the input gate
//! for both the wheel and `camera::zoom`), the mouse wheel, and the draggable
//! scrollbar (drag mapping + thumb sizing).

use bevy::input::mouse::MouseWheel;
use bevy::prelude::*;
use bevy::ui::{ComputedNode, UiGlobalTransform};
use bevy::window::PrimaryWindow;

use super::{ConsoleRoot, ConsoleView, ROWS, ScrollbarDrag, ScrollbarThumb, ScrollbarTrack};
use crate::console::{ConsoleHovered, ConsoleLog, clamp_offset};

/// Lines advanced per wheel notch.
const SCROLL_LINES: i64 = 3;
/// Smallest thumb height (fraction of the track) so it stays grabbable even with
/// a full 500-line buffer.
const MIN_THUMB_FRAC: f32 = 0.2;

/// Mirror the root's hover state into the shared resource that `camera::zoom`
/// and `console_scroll` read.
pub(super) fn console_hover(
    root: Query<&Interaction, With<ConsoleRoot>>,
    mut hovered: ResMut<ConsoleHovered>,
) {
    let over = root
        .single()
        .is_ok_and(|i| !matches!(i, Interaction::None));
    if hovered.0 != over {
        hovered.0 = over;
    }
}

pub(super) fn console_scroll(
    mut wheel: MessageReader<MouseWheel>,
    hovered: Res<ConsoleHovered>,
    log: Res<ConsoleLog>,
    mut view: ResMut<ConsoleView>,
) {
    // Always drain our reader so events never backlog into a late jump; only act
    // when the pointer is actually over the console.
    let delta: f32 = wheel.read().map(|e| e.y).sum();
    if !hovered.0 || delta == 0.0 {
        return;
    }
    // Wheel up (delta > 0) → older lines → smaller offset.
    let step = delta.round() as i64 * SCROLL_LINES;
    let new = (view.offset as i64 - step).max(0) as usize;
    view.offset = clamp_offset(new, log.lines().len(), ROWS);
    // Re-stick only when scrolled all the way back to the newest line.
    view.stick = view.offset >= log.lines().len().saturating_sub(ROWS);
}

/// Drags the view from the scrollbar thumb. Starts on a thumb press, continues
/// while the left button is held (even if the pointer leaves the thumb), ends on
/// release. Maps the physical cursor Y over the track's physical rect to the
/// offset — physical pixels throughout (`ComputedNode`/`UiGlobalTransform` and
/// `physical_cursor_position`), so the display scale factor cancels.
pub(super) fn scrollbar_drag(
    buttons: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    thumb: Query<&Interaction, With<ScrollbarThumb>>,
    track: Query<(&ComputedNode, &UiGlobalTransform), With<ScrollbarTrack>>,
    log: Res<ConsoleLog>,
    mut view: ResMut<ConsoleView>,
    mut drag: ResMut<ScrollbarDrag>,
) {
    if !buttons.pressed(MouseButton::Left) {
        // Guard the write: a blind `drag.0 = false` trips change detection
        // every frame the button is up (same anti-pattern as the old console
        // relayout cascade).
        if drag.0 {
            drag.0 = false;
        }
    } else if buttons.just_pressed(MouseButton::Left)
        && thumb.single().is_ok_and(|i| *i == Interaction::Pressed)
    {
        drag.0 = true;
    }
    if !drag.0 {
        return;
    }
    let len = log.lines().len();
    if len <= ROWS {
        return;
    }
    let (Ok((computed, xf)), Ok(window)) = (track.single(), windows.single()) else {
        return;
    };
    let Some(cursor) = window.physical_cursor_position() else {
        return;
    };
    let size = computed.size();
    if size.y <= 0.0 {
        return;
    }
    let track_top = xf.translation.y - size.y / 2.0;
    let rel = ((cursor.y - track_top) / size.y).clamp(0.0, 1.0);
    let new = (rel * (len - ROWS) as f32).round() as usize;
    view.offset = clamp_offset(new, len, ROWS);
    view.stick = view.offset >= len.saturating_sub(ROWS);
}

/// Sizes/places the scrollbar thumb from the current offset, and hides the whole
/// track when everything fits (`len <= ROWS`). Percentage-based, so no scale
/// factor enters; writes only on change to avoid needless relayout.
pub(super) fn update_scrollbar(
    log: Res<ConsoleLog>,
    view: Res<ConsoleView>,
    mut track: Query<&mut Node, (With<ScrollbarTrack>, Without<ScrollbarThumb>)>,
    mut thumb: Query<&mut Node, (With<ScrollbarThumb>, Without<ScrollbarTrack>)>,
) {
    let Ok(mut track_node) = track.single_mut() else { return };
    let len = log.lines().len();
    // Guard the division below: nothing to scroll → hide and bail.
    if len <= ROWS {
        if track_node.display != Display::None {
            track_node.display = Display::None;
        }
        return;
    }
    if track_node.display != Display::Flex {
        track_node.display = Display::Flex;
    }
    let thumb_frac = (ROWS as f32 / len as f32).max(MIN_THUMB_FRAC);
    let pos_frac = view.offset as f32 / (len - ROWS) as f32;
    let top_frac = (1.0 - thumb_frac) * pos_frac;
    let Ok(mut thumb_node) = thumb.single_mut() else { return };
    let height = Val::Percent(thumb_frac * 100.0);
    let top = Val::Percent(top_frac * 100.0);
    if thumb_node.height != height {
        thumb_node.height = height;
    }
    if thumb_node.top != top {
        thumb_node.top = top;
    }
}
