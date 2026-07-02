//! Main menu — the default screen the app boots into. "Start" hands off to
//! the [`GameState::Loading`] gate; "Beenden" quits. The language toggle lives
//! here too (first-screen access) and rebuilds the menu in place on change.

use bevy::app::AppExit;
use bevy::prelude::*;
use bevy::text::Font;

use super::encyclopedia::{ControlsButton, HelpButton, HelpOpen};
use super::widgets::{
    BUTTON_BG, BUTTON_BG_PRIMARY, TEXT_BRIGHT, TEXT_DIM, button, despawn_all, text_bundle,
};
use crate::font::UiFont;
use crate::i18n::{set_lang, t};
use crate::levels::Progress;
use crate::state::GameState;

#[derive(Component)]
struct UiMainMenu;

#[derive(Component, Clone, Copy)]
enum MenuAction {
    Start,
    Quit,
}

/// Language toggle on the main menu. Its own marker (not the level-select
/// `LangButton`) because it must rebuild THIS screen, not the route select.
#[derive(Component)]
struct MenuLangButton;

pub(super) struct MainMenuPlugin;

impl Plugin for MainMenuPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameState::MainMenu), spawn_main_menu)
            .add_systems(OnExit(GameState::MainMenu), despawn_all::<UiMainMenu>)
            .add_systems(
                Update,
                (menu_clicks, lang_click, menu_keys).run_if(in_state(GameState::MainMenu)),
            );
    }
}

fn spawn_main_menu(mut commands: Commands, ui_font: Res<UiFont>, progress: Res<Progress>) {
    spawn_menu(&mut commands, &ui_font.0, &progress);
}

/// Builds the whole main-menu tree. Split out so the language toggle can rebuild
/// it in place (despawn + this) so every string updates immediately.
fn spawn_menu(commands: &mut Commands, font: &Handle<Font>, progress: &Progress) {
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                row_gap: Val::Px(8.0),
                ..default()
            },
            UiMainMenu,
        ))
        .with_children(|root| {
            root.spawn(text_bundle(font, t("menu.title"), 56.0, TEXT_BRIGHT));
            root.spawn((
                text_bundle(font, t("menu.subtitle"), 16.0, TEXT_DIM),
                Node {
                    margin: UiRect::bottom(Val::Px(24.0)),
                    ..default()
                },
            ));
            button(root, font, &t("menu.start"), BUTTON_BG_PRIMARY, MenuAction::Start);
            button(root, font, &t("help.button"), BUTTON_BG, HelpButton);
            button(root, font, &t("controls.button"), BUTTON_BG, ControlsButton);
            button(root, font, &t("select.lang"), BUTTON_BG, MenuLangButton);
            button(root, font, &t("menu.quit"), BUTTON_BG, MenuAction::Quit);
            root.spawn((
                text_bundle(font, t("options.volume"), 16.0, TEXT_BRIGHT),
                Node {
                    margin: UiRect::top(Val::Px(24.0)),
                    ..default()
                },
            ));
            super::options::volume_controls(root, font, progress);
        });
}

/// Toggle the language on the main menu and rebuild it in place, so the title,
/// buttons and the toggle's own label all switch immediately (the level-select
/// toggle can't do this here — it rebuilds a different screen).
fn lang_click(
    interactions: Query<&Interaction, (Changed<Interaction>, With<MenuLangButton>)>,
    mut commands: Commands,
    ui_font: Res<UiFont>,
    mut progress: ResMut<Progress>,
    roots: Query<Entity, With<UiMainMenu>>,
) {
    if !interactions.iter().any(|i| *i == Interaction::Pressed) {
        return;
    }
    let new_lang = if progress.lang == "en" { "de" } else { "en" };
    progress.lang = new_lang.to_string();
    progress.save();
    set_lang(new_lang);
    for e in &roots {
        commands.entity(e).despawn();
    }
    spawn_menu(&mut commands, &ui_font.0, &progress);
}

fn menu_clicks(
    mut interactions: Query<(&Interaction, &MenuAction), Changed<Interaction>>,
    mut next: ResMut<NextState<GameState>>,
    mut exit: MessageWriter<AppExit>,
) {
    for (interaction, action) in &mut interactions {
        if *interaction != Interaction::Pressed {
            continue;
        }
        match action {
            MenuAction::Start => next.set(GameState::Loading),
            MenuAction::Quit => {
                exit.write(AppExit::Success);
            }
        }
    }
}

/// Keyboard shortcuts: Enter/Space start, Esc quits.
fn menu_keys(
    keys: Res<ButtonInput<KeyCode>>,
    help: Res<HelpOpen>,
    mut next: ResMut<NextState<GameState>>,
    mut exit: MessageWriter<AppExit>,
) {
    // The help overlay owns Esc/Enter while it is open.
    if help.0.is_some() {
        return;
    }
    if keys.just_pressed(KeyCode::Enter) || keys.just_pressed(KeyCode::Space) {
        next.set(GameState::Loading);
    }
    if keys.just_pressed(KeyCode::Escape) {
        exit.write(AppExit::Success);
    }
}
