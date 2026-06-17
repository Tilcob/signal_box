//! In-level log/error console: the rendering half of [`crate::console`]. A
//! fixed-size panel at the bottom-centre showing the last lines of `ConsoleLog`,
//! scrollable with the wheel while hovered.
//!
//! It is **virtualized**: a fixed pool of `ROWS` text nodes is spawned once and
//! never despawned — scrolling only changes which slice of the ring buffer they
//! display, updated in place. This deliberately avoids spawning/despawning text
//! nodes per change, which corrupts the glyph atlas in this build (see the
//! vendored `bevy_text`). The panel persists across Edit↔Run so the history
//! survives; visibility is toggled by state.

use bevy::input::mouse::MouseWheel;
use bevy::prelude::*;

use super::widgets::{PANEL_BG, TEXT_BRIGHT, text_bundle};
use crate::console::{ConsoleHovered, ConsoleLog, Severity, clamp_offset};
use crate::font::UiFont;
use crate::state::GameState;

/// Visible line count = size of the text-node pool.
const ROWS: usize = 10;
/// Lines advanced per wheel notch.
const SCROLL_LINES: i64 = 3;

const LOG_ERROR: Color = Color::srgb(1.0, 0.45, 0.35);
const LOG_WARN: Color = Color::srgb(1.0, 0.78, 0.35);
const LOG_INFO: Color = TEXT_BRIGHT;

#[derive(Component)]
struct ConsoleRoot;

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

pub(super) struct ConsoleUiPlugin;

impl Plugin for ConsoleUiPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ConsoleView>()
            // Spawned once on first entering a level; the query guard makes a
            // second entry a no-op, so the pool and history persist.
            .add_systems(OnEnter(GameState::Edit), ensure_console)
            .add_systems(OnEnter(GameState::Run), ensure_console)
            .add_systems(
                Update,
                (
                    console_visibility,
                    (console_hover, console_scroll, console_render).chain(),
                ),
            );
    }
}

fn ensure_console(mut commands: Commands, ui_font: Res<UiFont>, existing: Query<(), With<ConsoleRoot>>) {
    if !existing.is_empty() {
        return;
    }
    let font = ui_font.0.clone();
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                bottom: Val::Px(10.0),
                // Centre horizontally: 40 % wide, 30 % gap each side.
                left: Val::Percent(30.0),
                width: Val::Percent(40.0),
                height: Val::Px(184.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(1.0),
                padding: UiRect::all(Val::Px(6.0)),
                // Long lines clip instead of wrapping — wrapping would change a
                // row's height and shift the fixed layout.
                overflow: Overflow::clip(),
                ..default()
            },
            BackgroundColor(PANEL_BG),
            // Detectable hover (drives `ConsoleHovered`) and a click sink so the
            // board pointer doesn't fire underneath, like the other panels.
            Interaction::default(),
            Visibility::Hidden,
            ConsoleRoot,
        ))
        .with_children(|panel| {
            for _ in 0..ROWS {
                panel.spawn(text_bundle(&font, String::new(), 13.0, LOG_INFO));
            }
        });
}

/// Show the console only in-level (Edit/Run); hidden everywhere else.
fn console_visibility(
    state: Res<State<GameState>>,
    mut root: Query<&mut Visibility, With<ConsoleRoot>>,
) {
    if !state.is_changed() {
        return;
    }
    let Ok(mut vis) = root.single_mut() else { return };
    *vis = if matches!(**state, GameState::Edit | GameState::Run) {
        Visibility::Visible
    } else {
        Visibility::Hidden
    };
}

/// Mirror the root's hover state into the shared resource that `camera::zoom`
/// and `console_scroll` read.
fn console_hover(
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

fn console_scroll(
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

/// Maps the visible buffer slice onto the fixed row pool, in place. Cheap on
/// idle frames: a `(len, offset)` compare short-circuits when nothing moved.
fn console_render(
    log: Res<ConsoleLog>,
    mut view: ResMut<ConsoleView>,
    root: Query<&Children, With<ConsoleRoot>>,
    mut rows: Query<(&mut Text, &mut TextColor)>,
    mut last: Local<Option<(usize, usize)>>,
) {
    let Ok(children) = root.single() else { return };
    if view.stick {
        view.offset = log.lines().len().saturating_sub(ROWS);
    }
    let key = (log.lines().len(), view.offset);
    if *last == Some(key) && !log.is_changed() {
        return;
    }
    *last = Some(key);

    let lines = log.lines();
    for (i, child) in children.iter().take(ROWS).enumerate() {
        let Ok((mut text, mut color)) = rows.get_mut(child) else {
            continue;
        };
        match lines.get(view.offset + i) {
            Some(line) => {
                if text.0 != line.text {
                    text.0 = line.text.clone();
                }
                let c = severity_color(line.severity);
                if color.0 != c {
                    color.0 = c;
                }
            }
            None if !text.0.is_empty() => text.0.clear(),
            None => {}
        }
    }
}

fn severity_color(severity: Severity) -> Color {
    match severity {
        Severity::Error => LOG_ERROR,
        Severity::Warn => LOG_WARN,
        Severity::Info => LOG_INFO,
    }
}
