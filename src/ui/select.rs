//! Level select screen: catalog buttons with medals, sandbox entry, code
//! import and the language toggle.

use bevy::prelude::*;
use stellwerk_codes::Payload;

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
                button(
                    root,
                    &font,
                    &format!(
                        "{check}{}  {medal_str}",
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
            true,
            &progress,
            &mut commands,
            &mut editor,
            &mut next,
        );
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
                Err(e) => status.0 = format!("{e}"),
                Ok(Payload::Solution { level_id, layout }) => {
                    if level_id == SANDBOX_ID || catalog.0.iter().any(|entry| entry.id == level_id)
                    {
                        progress.entry(&level_id).layout = layout;
                        progress.save();
                        status.0 = format!("Lösung importiert: {level_id}");
                    } else {
                        status.0 = format!("Unbekanntes Level: {level_id}");
                    }
                }
                Ok(Payload::Level { level }) => {
                    save_sandbox(&level);
                    status.0 = format!("Level importiert (Sandbox): {}", level.name);
                }
            },
        }
    }
}
