//! In-level log/error console: the rendering half of [`crate::console`]. A
//! fixed-size panel (bottom-right) showing the last lines of `ConsoleLog`,
//! scrollable with the wheel or the right-hand scrollbar while hovered.
//!
//! It is **virtualized**: a fixed pool of `ROWS` text nodes (plus a scrollbar
//! thumb) is spawned once and never despawned — scrolling only changes which
//! slice of the ring buffer they display, updated in place. This deliberately
//! avoids spawning/despawning text nodes per change, which corrupts the glyph
//! atlas in this build (see the vendored `bevy_text`). The panel persists across
//! Edit↔Run so the history survives; visibility is toggled by state.
//!
//! Located lines (validation errors etc. carrying a board cell) are clickable —
//! they hover/press-highlight, play the UI click sound and recentre the camera.
//!
//! Split by responsibility: [`spawn`] (root lifecycle), [`scroll`] (hover gate +
//! wheel + scrollbar), [`render`] (buffer→pool mapping), [`interact`]
//! (click-to-jump + highlight). This file owns the plugin and the shared
//! component/resource/const vocabulary.

mod interact;
mod render;
mod scroll;
mod spawn;

use bevy::prelude::*;

use super::widgets::TEXT_BRIGHT;
use crate::console::ConsoleLog;
use crate::state::GameState;

/// Visible line count = size of the text-node pool.
const ROWS: usize = 6;
/// Default/info line colour — shared by the spawned row pool and
/// `render::severity_color`.
const LOG_INFO: Color = TEXT_BRIGHT;

#[derive(Component)]
struct ConsoleRoot;

/// One pooled text row; the index drives which buffer line it shows.
#[derive(Component)]
struct ConsoleRow(usize);

/// Per-row camera jump target, mirrored from the displayed line every render so
/// it tracks scrolling. `None` = the line is not clickable (no highlight/sound).
#[derive(Component, Default, PartialEq)]
struct RowJump(Option<Vec2>);

#[derive(Component)]
struct ScrollbarTrack;
#[derive(Component)]
struct ScrollbarThumb;

/// Scroll state: `offset` is the buffer index shown in the top row; `stick`
/// keeps the view pinned to the newest line until the player scrolls up.
#[derive(Resource)]
struct ConsoleView {
    offset: usize,
    stick: bool,
}

impl Default for ConsoleView {
    fn default() -> Self {
        Self {
            offset: 0,
            stick: true,
        }
    }
}

/// True while the scrollbar thumb is being dragged with the left mouse button.
#[derive(Resource, Default)]
struct ScrollbarDrag(bool);

pub(super) struct ConsoleUiPlugin;

impl Plugin for ConsoleUiPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ConsoleView>()
            .init_resource::<ScrollbarDrag>()
            // Spawned once on first entering a level; the query guard makes a
            // second entry a no-op, so the pool and history persist.
            .add_systems(OnEnter(GameState::Edit), spawn::ensure_console)
            .add_systems(OnEnter(GameState::Run), spawn::ensure_console)
            .add_systems(
                Update,
                (
                    spawn::console_visibility,
                    // `scrollbar_drag` before `console_render` so it clears
                    // `stick` before render can re-pin the view to the bottom.
                    (
                        scroll::console_hover,
                        scroll::console_scroll,
                        scroll::scrollbar_drag,
                        render::console_render,
                    )
                        .chain(),
                    interact::console_row_feedback,
                ),
            )
            .add_systems(
                Update,
                scroll::update_scrollbar.run_if(
                    resource_changed::<ConsoleView>.or(resource_changed::<ConsoleLog>),
                ),
            )
            // Only in-level: a hidden console row in another state can still
            // latch `Pressed`, which would move the camera off-screen.
            .add_systems(
                Update,
                interact::console_jump_clicks
                    .run_if(in_state(GameState::Edit).or(in_state(GameState::Run))),
            );
    }
}
