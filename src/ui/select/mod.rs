//! Level select screen. Split by responsibility:
//! - this file: the plugin, the shared component/resource vocabulary, the
//!   screen scaffolding (spawn/rebuild dispatch, status line, Esc hierarchy),
//! - [`views`]: the chapter overview + per-chapter level view and navigation,
//! - [`actions`]: sandbox entry, code import, language toggle,
//! - [`dev`]: the `feature = "dev"` authoring controls.

mod actions;
mod views;
#[cfg(feature = "dev")]
mod dev;

use bevy::prelude::*;

use super::encyclopedia::HelpOpen;
use super::widgets::{StatusText, TEXT_DIM, despawn_all, set_text, text_bundle};
use crate::font::UiFont;
use crate::levels::{Catalog, Progress};
use crate::state::GameState;

// Re-exported at the module root so the i18n coverage test keeps importing them
// from `crate::ui::select::…` regardless of which sub-file owns them.
#[cfg(test)]
pub(crate) use actions::{DECODE_ERROR_KEYS, SELECT_CHAPTER_KEYS};

// --- Shared vocabulary: markers + resources, used across the sub-modules ----

#[derive(Component)]
struct UiSelect;
#[derive(Component)]
struct LevelButton(usize);
/// Opens the level view for a chapter (the campaign `chapter` number).
#[derive(Component)]
struct ChapterButton(u8);
/// Returns from a chapter's level view to the chapter overview.
#[derive(Component)]
struct BackButton;
#[derive(Component)]
struct SandboxButton;
#[derive(Component)]
struct NewSandboxButton;
#[derive(Component)]
struct ImportButton;
#[derive(Component)]
struct LangButton;
/// Leaves the level select for the main menu (same target as Esc on the
/// overview), shown only on the chapter overview.
#[derive(Component)]
struct MainMenuButton;

/// Status line content (import results etc.).
#[derive(Resource, Default)]
struct UiStatus(String);

/// Which chapter's level view is open. `None` = the chapter overview (the
/// screen's home), `Some(n)` = the level list of chapter `n`. Drives which of
/// the two layouts [`build_select`] builds; navigation rebuilds in place.
#[derive(Resource, Default)]
struct OpenChapter(Option<u8>);

// --- Dev authoring: only built with feature `dev` ---------
/// Per-level delete (carries the level id, stable across re-indexing).
#[cfg(feature = "dev")]
#[derive(Component)]
struct DevDeleteLevel(String);
/// Open a campaign level in the sandbox editor (its sources/sinks/schedule
/// become editable) for in-place tweaking. Carries the catalog index, like
/// [`LevelButton`].
#[cfg(feature = "dev")]
#[derive(Component)]
struct DevOpenSandbox(usize);
/// Wipe all progress (builds, slots, solved, scores) — keeps the language.
#[cfg(feature = "dev")]
#[derive(Component)]
struct DevResetProgress;
/// Delete EVERY level from disk — two-click armed (see [`DevDeleteArmed`]).
#[cfg(feature = "dev")]
#[derive(Component)]
struct DevDeleteAll;
/// Arms the destructive "delete all" so a single misclick cannot fire it.
#[cfg(feature = "dev")]
#[derive(Resource, Default)]
struct DevDeleteArmed(bool);

pub(super) struct SelectUiPlugin;

impl Plugin for SelectUiPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<UiStatus>()
            .init_resource::<OpenChapter>()
            .add_systems(
                OnEnter(GameState::LevelSelect),
                (spawn_select, update_status).chain(),
            )
            .add_systems(
                OnExit(GameState::LevelSelect),
                (despawn_all::<UiSelect>, reset_open_chapter),
            )
            .add_systems(
                Update,
                (
                    views::click_level,
                    actions::select_buttons,
                    views::chapter_clicks,
                    views::back_click,
                    // Reads `HelpOpen` before the help overlay's Esc-close can
                    // flip it (see `encyclopedia::HelpEscClose`).
                    leave_to_menu.before(super::encyclopedia::HelpEscClose),
                )
                    .run_if(in_state(GameState::LevelSelect)),
            )
            // Status line changes only on explicit actions (import/lang) — no
            // need to rebuild + clone the string every frame.
            .add_systems(
                Update,
                update_status
                    .run_if(in_state(GameState::LevelSelect).and(resource_changed::<UiStatus>)),
            );
        #[cfg(feature = "dev")]
        app.init_resource::<DevDeleteArmed>().add_systems(
            Update,
            (dev::dev_select_actions, views::dev_open_sandbox)
                .run_if(in_state(GameState::LevelSelect)),
        );
    }
}

/// Esc hierarchy on the level select: the help overlay owns Esc first (closes
/// itself); then a chapter's level view (back to the overview); only an Esc on
/// the bare overview leaves to the main menu.
#[allow(clippy::too_many_arguments)]
fn leave_to_menu(
    keys: Res<ButtonInput<KeyCode>>,
    help: Res<HelpOpen>,
    roots: Query<Entity, With<UiSelect>>,
    ui_font: Res<UiFont>,
    catalog: Res<Catalog>,
    progress: Res<Progress>,
    status: Res<UiStatus>,
    mut open: ResMut<OpenChapter>,
    mut commands: Commands,
    mut next: ResMut<NextState<GameState>>,
) {
    if help.0.is_some() || !keys.just_pressed(KeyCode::Escape) {
        return;
    }
    if open.0.is_some() {
        open.0 = None;
        rebuild_select(&mut commands, &roots, &ui_font.0, &catalog, &progress, None, &status.0);
    } else {
        next.set(GameState::MainMenu);
    }
}

/// Always land on the overview when (re)entering the screen.
fn reset_open_chapter(mut open: ResMut<OpenChapter>) {
    open.0 = None;
}

fn spawn_select(
    mut commands: Commands,
    ui_font: Res<UiFont>,
    catalog: Res<Catalog>,
    progress: Res<Progress>,
    open: Res<OpenChapter>,
    status: Res<UiStatus>,
) {
    build_select(&mut commands, &ui_font.0, &catalog, &progress, open.0, &status.0);
}

/// Despawns the current screen and rebuilds it in place. Used by chapter
/// navigation and the dev authoring actions so they need not bounce through
/// another state — the only way to refresh the screen mid-`LevelSelect`.
#[allow(clippy::too_many_arguments)]
fn rebuild_select(
    commands: &mut Commands,
    roots: &Query<Entity, With<UiSelect>>,
    font: &Handle<Font>,
    catalog: &Catalog,
    progress: &Progress,
    open_chapter: Option<u8>,
    status: &str,
) {
    for e in roots {
        commands.entity(e).despawn();
    }
    build_select(commands, font, catalog, progress, open_chapter, status);
}

/// Builds the level-select screen for the current view: the chapter overview
/// (`open_chapter == None`) or one chapter's level list. `status` is passed in
/// (not left to `update_status`) so an import/language message survives an
/// in-place rebuild — the rebuild path does not re-run `update_status`.
fn build_select(
    commands: &mut Commands,
    font: &Handle<Font>,
    catalog: &Catalog,
    progress: &Progress,
    open_chapter: Option<u8>,
    status: &str,
) {
    let font = font.clone();
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                row_gap: Val::Px(5.0),
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.6)),
            UiSelect,
        ))
        .with_children(|root| {
            match open_chapter {
                None => views::build_overview(root, &font, catalog, progress),
                Some(ch) => views::build_chapter_view(root, &font, catalog, progress, ch),
            }
            root.spawn((text_bundle(&font, status.to_string(), 14.0, TEXT_DIM), StatusText));
        });
}

fn update_status(status: Res<UiStatus>, mut texts: Query<&mut Text, With<StatusText>>) {
    if let Ok(mut text) = texts.single_mut() {
        set_text(&mut text, status.0.clone());
    }
}
