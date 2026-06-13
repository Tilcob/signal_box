//! Level select screen: catalog buttons with medals, sandbox entry, code
//! import and the language toggle.

use bevy::prelude::*;
use bevy::text::Font;
use stellwerk_codes::{DecodeError, Payload};

use super::enter_level;
#[cfg(feature = "dev")]
use super::widgets::small_button;
use super::widgets::{
    BUTTON_BG, BUTTON_BG_PRIMARY, StatusText, TEXT_BRIGHT, TEXT_DIM, button, despawn_all,
    set_text, text_bundle,
};
use crate::font::UiFont;
use crate::i18n::{level_name, set_lang, t};
use crate::levels::{Catalog, Progress, SANDBOX_ID, load_sandbox, save_sandbox};
#[cfg(feature = "dev")]
use crate::levels::load_catalog;
use crate::state::{Editor, GameState};

#[derive(Component)]
struct UiSelect;
#[derive(Component)]
struct LevelButton(usize);
#[derive(Component)]
struct SandboxButton;
#[derive(Component)]
struct NewSandboxButton;
#[derive(Component)]
struct ImportButton;
#[derive(Component)]
struct LangButton;

/// Status line content (import results etc.).
#[derive(Resource, Default)]
struct UiStatus(String);

// --- Dev authoring (optimierung/07): only built with feature `dev` ---------
/// Per-level delete (carries the level id, stable across re-indexing).
#[cfg(feature = "dev")]
#[derive(Component)]
struct DevDeleteLevel(String);
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
            .add_systems(OnEnter(GameState::LevelSelect), spawn_select)
            .add_systems(OnExit(GameState::LevelSelect), despawn_all::<UiSelect>)
            .add_systems(
                Update,
                (click_level, select_buttons, update_status, leave_to_menu)
                    .run_if(in_state(GameState::LevelSelect)),
            );
        #[cfg(feature = "dev")]
        app.init_resource::<DevDeleteArmed>().add_systems(
            Update,
            dev_select_actions.run_if(in_state(GameState::LevelSelect)),
        );
    }
}

/// Esc returns to the main menu.
fn leave_to_menu(keys: Res<ButtonInput<KeyCode>>, mut next: ResMut<NextState<GameState>>) {
    if keys.just_pressed(KeyCode::Escape) {
        next.set(GameState::MainMenu);
    }
}

fn spawn_select(
    mut commands: Commands,
    ui_font: Res<UiFont>,
    catalog: Res<Catalog>,
    progress: Res<Progress>,
) {
    build_select(&mut commands, &ui_font.0, &catalog, &progress);
}

/// Builds the whole level-select screen. Extracted from the `OnEnter` system so
/// the dev authoring actions can rebuild it in place (after writing/deleting a
/// level) without bouncing through another state.
fn build_select(
    commands: &mut Commands,
    font: &Handle<Font>,
    catalog: &Catalog,
    progress: &Progress,
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
            root.spawn(text_bundle(&font, t("select.title"), 30.0, TEXT_BRIGHT));
            root.spawn(text_bundle(&font, t("select.hint"), 14.0, TEXT_DIM));
            for (index, entry) in catalog.0.iter().enumerate() {
                let progress_entry = progress.levels.get(&entry.id);
                let medals = progress_entry
                    .map(|p| p.medals(&entry.level))
                    .unwrap_or_default();
                let solved = progress_entry.is_some_and(|p| p.solved);
                let medal_str: String =
                    medals.iter().map(|m| if *m { '●' } else { '○' }).collect();
                let check = if solved { "✓ " } else { "   " };
                let hard = if entry.meta.optional_hard {
                    format!("  {}", t("select.optional_hard"))
                } else {
                    String::new()
                };
                let label = format!(
                    "{check}{}  {medal_str}{hard}",
                    level_name(&entry.id, &entry.level.name)
                );
                // Dev: a per-level delete sits right beside each level button.
                #[cfg(feature = "dev")]
                root.spawn(Node {
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    ..default()
                })
                .with_children(|r| {
                    button(r, &font, &label, BUTTON_BG, LevelButton(index));
                    small_button(r, &font, "🗑", DevDeleteLevel(entry.id.clone()));
                });
                #[cfg(not(feature = "dev"))]
                button(root, &font, &label, BUTTON_BG, LevelButton(index));
            }
            root.spawn(Node {
                flex_direction: FlexDirection::Row,
                margin: UiRect::top(Val::Px(10.0)),
                ..default()
            })
            .with_children(|row| {
                button(
                    row,
                    &font,
                    &t("select.sandbox"),
                    BUTTON_BG_PRIMARY,
                    SandboxButton,
                );
                button(
                    row,
                    &font,
                    &t("select.new_sandbox"),
                    BUTTON_BG,
                    NewSandboxButton,
                );
                button(row, &font, &t("select.import"), BUTTON_BG, ImportButton);
                button(row, &font, &t("select.lang"), BUTTON_BG, LangButton);
            });
            // Dev authoring controls (never in a ship build).
            #[cfg(feature = "dev")]
            root.spawn(Node {
                flex_direction: FlexDirection::Row,
                margin: UiRect::top(Val::Px(6.0)),
                ..default()
            })
            .with_children(|row| {
                button(
                    row,
                    &font,
                    "DEV: Fortschritt zurücksetzen",
                    BUTTON_BG,
                    DevResetProgress,
                );
                button(row, &font, "DEV: ALLE Level löschen", BUTTON_BG, DevDeleteAll);
            });
            root.spawn((text_bundle(&font, String::new(), 14.0, TEXT_DIM), StatusText));
        });
}

fn update_status(status: Res<UiStatus>, mut texts: Query<&mut Text, With<StatusText>>) {
    if let Ok(mut text) = texts.single_mut() {
        set_text(&mut text, status.0.clone());
    }
}

fn click_level(
    mut interactions: Query<(&Interaction, &LevelButton), Changed<Interaction>>,
    catalog: Res<Catalog>,
    progress: Res<Progress>,
    mut commands: Commands,
    mut editor: ResMut<Editor>,
    mut next: ResMut<NextState<GameState>>,
) {
    for (interaction, level_button) in &mut interactions {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let entry = &catalog.0[level_button.0];
        enter_level(
            level_button.0,
            entry.id.clone(),
            entry.level.clone(),
            entry.meta.briefing.clone(),
            false,
            &progress,
            &mut commands,
            &mut editor,
            &mut next,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn select_buttons(
    sandbox: Query<&Interaction, (Changed<Interaction>, With<SandboxButton>)>,
    new_sandbox: Query<&Interaction, (Changed<Interaction>, With<NewSandboxButton>)>,
    import: Query<&Interaction, (Changed<Interaction>, With<ImportButton>)>,
    lang: Query<&Interaction, (Changed<Interaction>, With<LangButton>)>,
    catalog: Res<Catalog>,
    mut progress: ResMut<Progress>,
    mut status: ResMut<UiStatus>,
    mut commands: Commands,
    mut editor: ResMut<Editor>,
    mut next: ResMut<NextState<GameState>>,
) {
    if sandbox.iter().any(|i| *i == Interaction::Pressed) {
        let level = load_sandbox();
        enter_level(
            usize::MAX,
            SANDBOX_ID.to_string(),
            level,
            String::new(),
            true,
            &progress,
            &mut commands,
            &mut editor,
            &mut next,
        );
        return;
    }
    if new_sandbox.iter().any(|i| *i == Interaction::Pressed) {
        next.set(GameState::SandboxSetup);
        return;
    }
    if lang.iter().any(|i| *i == Interaction::Pressed) {
        let new_lang = if progress.lang == "en" { "de" } else { "en" };
        progress.lang = new_lang.to_string();
        progress.save();
        set_lang(new_lang);
        // Rebuild the screen with the new language.
        next.set(GameState::LevelSelect);
        status.0 = t("select.lang");
        return;
    }
    if import.iter().any(|i| *i == Interaction::Pressed) {
        match std::fs::read_to_string("stellwerk_import.txt") {
            Err(e) => status.0 = format!("stellwerk_import.txt: {e}"),
            Ok(text) => match stellwerk_codes::decode(&text) {
                Err(e) => status.0 = decode_error_text(&e),
                Ok(Payload::Solution { level_id, layout }) => {
                    if level_id == SANDBOX_ID || catalog.0.iter().any(|entry| entry.id == level_id)
                    {
                        progress.entry(&level_id).layout = layout;
                        progress.save();
                        status.0 = format!("{}{level_id}", t("select.import_ok"));
                    } else {
                        status.0 = format!("{}{level_id}", t("select.import_unknown"));
                    }
                }
                Ok(Payload::Level { level }) => {
                    save_sandbox(&level);
                    status.0 = format!("{}{}", t("select.import_sandbox"), level.name);
                }
            },
        }
    }
}

/// Every key [`decode_error_text`] can emit — kept beside the match so the
/// i18n coverage checker (see `crate::i18n` tests) asserts all of them resolve
/// in both languages. Adding a [`DecodeError`] variant breaks the exhaustive
/// match below and reminds you to extend this.
#[cfg(test)]
pub(crate) const DECODE_ERROR_KEYS: &[&str] = &[
    "import.error.prefix",
    "import.error.base64",
    "import.error.version",
    "import.error.corrupt",
];

/// Localized import-failure text. `DecodeError`'s own `Display` stays English
/// (logs); the player-facing message is translated here — same split as
/// `edit_hud::valerr_text` for `ValidationError`.
pub(crate) fn decode_error_text(e: &DecodeError) -> String {
    match e {
        DecodeError::Prefix => t("import.error.prefix"),
        DecodeError::Base64 => t("import.error.base64"),
        DecodeError::Version(v) => format!("{} ({v})", t("import.error.version")),
        DecodeError::Corrupt => t("import.error.corrupt"),
    }
}

/// Dev authoring (optimierung/07): per-level delete, full progress reset, and a
/// two-click "delete all". After any mutation the catalog and i18n table are
/// reloaded and the screen is rebuilt in place. Only compiled with `dev`.
#[cfg(feature = "dev")]
#[allow(clippy::too_many_arguments)]
fn dev_select_actions(
    per_level: Query<(&Interaction, &DevDeleteLevel), Changed<Interaction>>,
    reset: Query<&Interaction, (Changed<Interaction>, With<DevResetProgress>)>,
    delete_all: Query<&Interaction, (Changed<Interaction>, With<DevDeleteAll>)>,
    roots: Query<Entity, With<UiSelect>>,
    ui_font: Res<UiFont>,
    mut catalog: ResMut<Catalog>,
    mut progress: ResMut<Progress>,
    mut status: ResMut<UiStatus>,
    mut armed: ResMut<DevDeleteArmed>,
    mut commands: Commands,
) {
    let mut dirty = false;
    let mut catalog_changed = false;

    for (interaction, target) in &per_level {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let id = target.0.clone();
        crate::authoring::delete_level(&id);
        progress.levels.remove(&id);
        progress.save();
        status.0 = format!("Level gelöscht: {id}");
        armed.0 = false;
        dirty = true;
        catalog_changed = true;
    }

    if reset.iter().any(|i| *i == Interaction::Pressed) {
        let lang = progress.lang.clone();
        *progress = Progress::default();
        progress.lang = lang;
        progress.save();
        status.0 = "Fortschritt zurückgesetzt (Gleise/Weichen/Scores aller Level).".into();
        armed.0 = false;
        dirty = true;
    }

    if delete_all.iter().any(|i| *i == Interaction::Pressed) {
        if armed.0 {
            let ids: Vec<String> = catalog.0.iter().map(|e| e.id.clone()).collect();
            for id in &ids {
                crate::authoring::delete_level(id);
                progress.levels.remove(id);
            }
            progress.save();
            status.0 = format!("{} Level von der Platte gelöscht.", ids.len());
            armed.0 = false;
            dirty = true;
            catalog_changed = true;
        } else {
            armed.0 = true;
            status.0 = "⚠ Nochmal „ALLE Level löschen\" klicken zum Bestätigen.".into();
        }
    }

    if !dirty {
        return;
    }
    // Level deletes remove i18n keys; reload the live table for the current
    // language so the rebuilt screen reflects the change.
    let lang = if progress.lang.is_empty() {
        "de"
    } else {
        &progress.lang
    };
    set_lang(lang);
    if catalog_changed {
        *catalog = load_catalog();
    }
    for e in &roots {
        commands.entity(e).despawn();
    }
    build_select(&mut commands, &ui_font.0, &catalog, &progress);
}
