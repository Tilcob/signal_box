//! Main menu — the default screen the app boots into. "Start" hands off to
//! the [`GameState::Loading`] gate; "Beenden" quits.

use bevy::app::AppExit;
use bevy::prelude::*;

use super::encyclopedia::{HelpButton, HelpOpen};
use super::widgets::{
    BUTTON_BG, BUTTON_BG_PRIMARY, TEXT_BRIGHT, TEXT_DIM, button, despawn_all, text_bundle,
};
use crate::font::UiFont;
use crate::i18n::t;
use crate::state::GameState;

#[derive(Component)]
struct UiMainMenu;

#[derive(Component, Clone, Copy)]
enum MenuAction {
    Start,
    Quit,
}

pub(super) struct MainMenuPlugin;

impl Plugin for MainMenuPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameState::MainMenu), spawn_main_menu)
            .add_systems(OnExit(GameState::MainMenu), despawn_all::<UiMainMenu>)
            .add_systems(
                Update,
                (menu_clicks, menu_keys).run_if(in_state(GameState::MainMenu)),
            );
    }
}

fn spawn_main_menu(mut commands: Commands, ui_font: Res<UiFont>) {
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
                row_gap: Val::Px(8.0),
                ..default()
            },
            UiMainMenu,
        ))
        .with_children(|root| {
            root.spawn(text_bundle(&font, t("menu.title"), 56.0, TEXT_BRIGHT));
            root.spawn((
                text_bundle(&font, t("menu.subtitle"), 16.0, TEXT_DIM),
                Node {
                    margin: UiRect::bottom(Val::Px(24.0)),
                    ..default()
                },
            ));
            button(root, &font, &t("menu.start"), BUTTON_BG_PRIMARY, MenuAction::Start);
            button(root, &font, &t("help.button"), BUTTON_BG, HelpButton);
            button(root, &font, &t("menu.quit"), BUTTON_BG, MenuAction::Quit);
        });
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
    if help.0 {
        return;
    }
    if keys.just_pressed(KeyCode::Enter) || keys.just_pressed(KeyCode::Space) {
        next.set(GameState::Loading);
    }
    if keys.just_pressed(KeyCode::Escape) {
        exit.write(AppExit::Success);
    }
}
