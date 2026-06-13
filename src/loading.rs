//! Loading state: the on-demand load gate between the main menu and the
//! game. Entered when the player presses "Start" in the main menu, shows a
//! loading screen, performs the (currently instant) catalog load and then
//! auto-advances to [`GameState::LevelSelect`].
//!
//! Why hand-rolled and not `bevy_asset_loader`: the only loadable payload
//! today is the level catalog, which is plain `std::fs` RON (the sim crate
//! is engine-free, so levels are not Bevy assets). The font and i18n must be
//! eager anyway — the loading screen itself renders text. `bevy_asset_loader`
//! only pays off once M3 adds real Bevy assets (audio, Pult capsule images);
//! at that point add a `GameAssets` collection here and gate on it too.

use bevy::prelude::*;

use crate::font::UiFont;
use crate::i18n::t;
use crate::levels::{Catalog, load_catalog};
use crate::state::GameState;

const TEXT_BRIGHT: Color = Color::srgb(0.88, 0.90, 0.95);

/// Loading-screen UI root (despawned on leaving [`GameState::Loading`]).
#[derive(Component)]
struct LoadingScreen;

pub struct LoadingPlugin;

impl Plugin for LoadingPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            OnEnter(GameState::Loading),
            (spawn_loading_screen, load_assets),
        )
        .add_systems(OnExit(GameState::Loading), despawn_loading_screen)
        .add_systems(Update, finish_loading.run_if(in_state(GameState::Loading)));
    }
}

fn spawn_loading_screen(mut commands: Commands, ui_font: Res<UiFont>) {
    let font = ui_font.0.clone();
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },
            LoadingScreen,
        ))
        .with_children(|root| {
            root.spawn((
                Text::new(t("loading.text")),
                TextFont {
                    font,
                    font_size: 28.0,
                    ..default()
                },
                TextColor(TEXT_BRIGHT),
            ));
        });
}

/// Performs the actual load. Instant today (a few tiny RON files); kept as
/// its own system so M3 can add asset-collection loading alongside it.
fn load_assets(mut commands: Commands) {
    commands.insert_resource(load_catalog());
}

/// Once everything is resident, hand off to the level select. Waiting for the
/// `Catalog` resource gives the loading screen at least one rendered frame and
/// guarantees `spawn_select` (which reads `Catalog`) finds it.
fn finish_loading(catalog: Option<Res<Catalog>>, mut next: ResMut<NextState<GameState>>) {
    if catalog.is_some() {
        next.set(GameState::LevelSelect);
    }
}

fn despawn_loading_screen(mut commands: Commands, q: Query<Entity, With<LoadingScreen>>) {
    for e in &q {
        commands.entity(e).despawn();
    }
}
