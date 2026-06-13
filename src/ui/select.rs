//! Level select screen: catalog buttons with medals, sandbox entry, code
//! import and the language toggle.

use bevy::prelude::*;
use stellwerk_codes::{DecodeError, Payload};

use super::enter_level;
use super::widgets::{
    BUTTON_BG, BUTTON_BG_PRIMARY, StatusText, TEXT_BRIGHT, TEXT_DIM, button, despawn_all,
    set_text, text_bundle,
};
use crate::font::UiFont;
use crate::i18n::{level_name, set_lang, t};
use crate::levels::{Catalog, Progress, SANDBOX_ID, load_sandbox, save_sandbox};
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
                let medal_str: String = medals.iter().map(|m| if *m { '●' } else { '○' }).collect();
                let check = if solved { "✓ " } else { "   " };
                let hard = if entry.meta.optional_hard {
                    format!("  {}", t("select.optional_hard"))
                } else {
                    String::new()
                };
                button(
                    root,
                    &font,
                    &format!(
                        "{check}{}  {medal_str}{hard}",
                        level_name(&entry.id, &entry.level.name)
                    ),
                    BUTTON_BG,
                    LevelButton(index),
                );
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
