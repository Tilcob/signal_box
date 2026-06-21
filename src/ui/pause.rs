//! In-level pause menu: an overlay reachable with Esc in Edit and Run. It
//! replaces the old "Esc leaves the level immediately" behaviour — Esc now
//! opens this menu, which offers Resume and Leave. Leave autosaves the build
//! (and the sandbox level) and returns to the level select.
//!
//! While the menu is open the gameplay input is gated off via
//! [`crate::state::not_paused`] so nothing happens behind it; in Run the sim
//! freezes because its tick system is gated by the same condition.

use bevy::prelude::*;

use super::widgets::{BUTTON_BG, BUTTON_BG_PRIMARY, PANEL_BG, TEXT_BRIGHT, button, text_bundle};
use crate::font::UiFont;
use crate::i18n::t;
use crate::levels::{Progress, save_sandbox};
use crate::state::{ActiveLevel, Editor, GameState, Paused};

#[derive(Component)]
struct PauseRoot;
#[derive(Component)]
struct ResumeButton;
#[derive(Component)]
struct LeaveButton;
#[derive(Component)]
struct EditorButton;

/// Pause-menu i18n keys, asserted present in both language tables by the
/// `crate::i18n` coverage test.
#[cfg(test)]
pub(crate) const PAUSE_KEYS: &[&str] =
    &["pause.title", "pause.resume", "pause.editor", "pause.leave"];

pub(super) struct PausePlugin;

impl Plugin for PausePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (resume_click, leave_click, back_editor_click, sync_overlay)
                .run_if(in_state(GameState::Edit).or(in_state(GameState::Run))),
        )
        // Leaving a level (via Leave below, a started run, or a finished run)
        // resets the flag and tears the overlay down with the screen.
        .add_systems(OnExit(GameState::Edit), teardown)
        .add_systems(OnExit(GameState::Run), teardown);
    }
}

/// Esc toggles the pause menu — it opens in place of the old "leave the level"
/// behaviour and closes again when already open. Yields to an open radial track
/// menu (which owns Esc to close itself first); in Run the radial is never open.
///
/// Registered inside the edit chain (before `radial::radial_menu`) and in the
/// run update set so the ordering against the radial holds; it stays ungated by
/// [`crate::state::not_paused`] because it is also the only way back out.
pub(crate) fn toggle_pause(
    keys: Res<ButtonInput<KeyCode>>,
    editor: Res<Editor>,
    mut paused: ResMut<Paused>,
) {
    if !keys.just_pressed(KeyCode::Escape) {
        return;
    }
    if editor.radial.is_some() {
        return;
    }
    paused.0 = !paused.0;
}

fn resume_click(
    interactions: Query<&Interaction, (Changed<Interaction>, With<ResumeButton>)>,
    mut paused: ResMut<Paused>,
) {
    if interactions.iter().any(|i| *i == Interaction::Pressed) {
        paused.0 = false;
    }
}

/// "Back to editor": jump straight from a paused Run into Edit. The build lives
/// in the `Editor` resource (untouched by the run), and every `OnEnter(Edit)`
/// system tears the run down and rebuilds the editor board — so this only has
/// to flip the state. Shown only in Run (see `sync_overlay`).
fn back_editor_click(
    interactions: Query<&Interaction, (Changed<Interaction>, With<EditorButton>)>,
    mut paused: ResMut<Paused>,
    mut next: ResMut<NextState<GameState>>,
) {
    if interactions.iter().any(|i| *i == Interaction::Pressed) {
        paused.0 = false;
        next.set(GameState::Edit);
    }
}

/// "Leave level": autosave the build (and the sandbox level) exactly as the old
/// Esc exit did, then return to the level select — from both Edit and Run.
fn leave_click(
    interactions: Query<&Interaction, (Changed<Interaction>, With<LeaveButton>)>,
    active: Option<Res<ActiveLevel>>,
    editor: Res<Editor>,
    mut progress: ResMut<Progress>,
    mut paused: ResMut<Paused>,
    mut next: ResMut<NextState<GameState>>,
) {
    if !interactions.iter().any(|i| *i == Interaction::Pressed) {
        return;
    }
    if let Some(active) = active {
        progress.entry(&active.id).layout = editor.layout.clone();
        progress.save();
        if active.sandbox {
            save_sandbox(&active.level);
        }
    }
    paused.0 = false;
    next.set(GameState::LevelSelect);
}

fn teardown(
    mut commands: Commands,
    mut paused: ResMut<Paused>,
    roots: Query<Entity, With<PauseRoot>>,
) {
    paused.0 = false;
    for e in &roots {
        commands.entity(e).despawn();
    }
}

/// Spawns/despawns the overlay when [`Paused`] changes (same pattern as the
/// help overlay's `sync_overlay`).
fn sync_overlay(
    mut commands: Commands,
    paused: Res<Paused>,
    ui_font: Res<UiFont>,
    state: Res<State<GameState>>,
    roots: Query<Entity, With<PauseRoot>>,
) {
    if !paused.is_changed() {
        return;
    }
    for e in &roots {
        commands.entity(e).despawn();
    }
    if !paused.0 {
        return;
    }
    let font = ui_font.0.clone();
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.75)),
            // Absorb clicks so they do not fall through to the board behind.
            Interaction::default(),
            PauseRoot,
        ))
        .with_children(|root| {
            root.spawn((
                Node {
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    min_width: Val::Px(280.0),
                    padding: UiRect::all(Val::Px(18.0)),
                    row_gap: Val::Px(10.0),
                    ..default()
                },
                BackgroundColor(PANEL_BG),
            ))
            .with_children(|panel| {
                panel.spawn((
                    text_bundle(&font, t("pause.title"), 28.0, TEXT_BRIGHT),
                    Node {
                        margin: UiRect::bottom(Val::Px(6.0)),
                        ..default()
                    },
                ));
                button(panel, &font, &t("pause.resume"), BUTTON_BG_PRIMARY, ResumeButton);
                // Run → Edit: only meaningful while a simulation runs; hidden in Edit.
                if *state.get() == GameState::Run {
                    button(panel, &font, &t("pause.editor"), BUTTON_BG, EditorButton);
                }
                button(panel, &font, &t("pause.leave"), BUTTON_BG, LeaveButton);
            });
        });
}
