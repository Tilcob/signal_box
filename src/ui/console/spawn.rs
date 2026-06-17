//! Console root lifecycle: the one-shot pool/layout spawn and the in-level
//! show/hide visibility toggle — the root entity's existence and visibility.

use bevy::prelude::*;
use bevy::ui::FocusPolicy;

use super::{ConsoleRoot, ConsoleRow, LOG_INFO, ROWS, RowJump, ScrollbarThumb, ScrollbarTrack};
use crate::font::UiFont;
use crate::state::GameState;
use crate::ui::widgets::{PANEL_BG, text_bundle};

/// Scrollbar track width.
const TRACK_W: f32 = 8.0;
const TRACK_BG: Color = Color::srgba(1.0, 1.0, 1.0, 0.05);
const THUMB_BG: Color = Color::srgba(1.0, 1.0, 1.0, 0.28);

pub(super) fn ensure_console(
    mut commands: Commands,
    ui_font: Res<UiFont>,
    existing: Query<(), With<ConsoleRoot>>,
) {
    if !existing.is_empty() {
        return;
    }
    let font = ui_font.0.clone();
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                bottom: Val::Px(10.0),
                // Compact panel shifted toward the bottom-right (30 % wide, right
                // edge 30 % from the right), clear of the dev save panel.
                right: Val::Percent(30.0),
                width: Val::Percent(30.0),
                height: Val::Px(104.0),
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(4.0),
                padding: UiRect::all(Val::Px(6.0)),
                ..default()
            },
            BackgroundColor(PANEL_BG),
            // Block focus so the panel stays a click sink (the board pointer
            // doesn't fire underneath) AND so hovering a `Pass` row still marks
            // the root hovered for `ConsoleHovered`.
            FocusPolicy::Block,
            Interaction::default(),
            Visibility::Hidden,
            ConsoleRoot,
        ))
        .with_children(|root| {
            // Content column: the text rows.
            root.spawn(Node {
                flex_grow: 1.0,
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(1.0),
                // Long lines clip instead of wrapping — wrapping would change a
                // row's height and shift the fixed layout.
                overflow: Overflow::clip(),
                ..default()
            })
            .with_children(|content| {
                for i in 0..ROWS {
                    // Full-width hit target with a manual `Interaction` (NOT a
                    // `Button`, so the global button feedback/click-sfx skip it —
                    // highlight + sound are driven only for located rows).
                    content.spawn((
                        text_bundle(&font, String::new(), 13.0, LOG_INFO),
                        Interaction::default(),
                        ConsoleRow(i),
                        RowJump(None),
                        BackgroundColor(Color::NONE),
                        Node {
                            width: Val::Percent(100.0),
                            ..default()
                        },
                    ));
                }
            });
            // Scrollbar track (right) with its draggable thumb.
            root.spawn((
                Node {
                    width: Val::Px(TRACK_W),
                    ..default()
                },
                BackgroundColor(TRACK_BG),
                ScrollbarTrack,
            ))
            .with_children(|track| {
                track.spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        width: Val::Percent(100.0),
                        height: Val::Percent(100.0),
                        top: Val::Px(0.0),
                        ..default()
                    },
                    BackgroundColor(THUMB_BG),
                    Interaction::default(),
                    ScrollbarThumb,
                ));
            });
        });
}

/// Show the console only in-level (Edit/Run); hidden everywhere else.
pub(super) fn console_visibility(
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
