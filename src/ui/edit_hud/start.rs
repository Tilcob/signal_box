//! The START button (top-right of the edit HUD) and the Space/click handling
//! that launches the run — or, when the build cannot run yet, logs the first
//! blocking problem to the console. Spawns its own root, tagged `UiEdit` so the
//! edit HUD's `despawn_all::<UiEdit>` cleans it on exit.

use bevy::prelude::*;

use super::UiEdit;
use crate::console::ConsoleLog;
use crate::font::UiFont;
use crate::i18n::t;
use crate::state::{Diagnostics, GameState, no_field_focused, not_paused};
use crate::ui::valerr::{build_issue_text, valerr_text};
use crate::ui::widgets::{BUTTON_BG_BLOCKED, BUTTON_BG_PRIMARY, ButtonBase, button, set_text};

#[derive(Component)]
struct StartButton;

pub(super) struct StartPlugin;

impl Plugin for StartPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameState::Edit), spawn_start_button)
            .add_systems(
                Update,
                // Gated so Space/Enter cannot start the run behind the pause
                // menu (the overlay already absorbs the start button click), nor
                // while typing into a numeric field (Enter commits the field).
                start_button
                    .run_if(not_paused)
                    .run_if(no_field_focused)
                    .run_if(in_state(GameState::Edit)),
            );
    }
}

fn spawn_start_button(mut commands: Commands, ui_font: Res<UiFont>) {
    let font = ui_font.0.clone();
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                right: Val::Px(10.0),
                top: Val::Px(8.0),
                ..default()
            },
            Interaction::default(),
            UiEdit,
        ))
        .with_children(|c| {
            button(c, &font, &t("edit.start"), BUTTON_BG_PRIMARY, StartButton);
        });
}

fn start_button(
    mut interactions: Query<(&Interaction, &mut ButtonBase, &Children), With<StartButton>>,
    mut texts: Query<&mut Text>,
    keys: Res<ButtonInput<KeyCode>>,
    diagnostics: Res<Diagnostics>,
    mut next: ResMut<NextState<GameState>>,
    mut log: ResMut<ConsoleLog>,
    mut mouse_was_pressed: Local<bool>,
) {
    let allowed = diagnostics.start_allowed();
    let mut mouse_pressed = false;
    for (interaction, mut base, children) in &mut interactions {
        let target = if allowed {
            BUTTON_BG_PRIMARY
        } else {
            BUTTON_BG_BLOCKED
        };
        // Write through ButtonBase (not BackgroundColor directly) so the
        // hover/press feedback keeps working — and only on actual change.
        if base.0 != target {
            base.0 = target;
        }
        if let Some(&child) = children.first()
            && let Ok(mut text) = texts.get_mut(child)
        {
            set_text(
                &mut text,
                if allowed {
                    t("edit.start")
                } else {
                    t("edit.start_blocked")
                },
            );
        }
        if *interaction == Interaction::Pressed {
            mouse_pressed = true;
        }
    }
    // Edge-detect the mouse press so a held button logs once, not per frame
    // (the keyboard keys are already edge-triggered).
    let mouse_edge = mouse_pressed && !*mouse_was_pressed;
    *mouse_was_pressed = mouse_pressed;
    // Space + the button only — NOT Enter. Enter commits a focused field and
    // clears its focus in the same frame, so an Enter that ran before this
    // system would also start the run (no_field_focused would already be true).
    // Space is safe: it never clears field focus, so a focused field gates it.
    let clicked = keys.just_pressed(KeyCode::Space) || mouse_edge;
    if !clicked {
        return;
    }
    if allowed {
        next.set(GameState::Run);
    } else if let Some(msg) = first_blocking_message(&diagnostics) {
        log.error(msg);
    }
}

/// The first build block / validation error, localized — the line the console
/// shows when the player hits START on a level that cannot run yet.
fn first_blocking_message(diagnostics: &Diagnostics) -> Option<String> {
    if let Some(issue) = diagnostics.build_issues.first() {
        Some(build_issue_text(issue))
    } else {
        diagnostics.errors.first().map(valerr_text)
    }
}
