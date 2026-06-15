//! Tool encyclopedia: a help overlay reachable from the main menu and the
//! level select ("outside the levels"). For now it is plain text — one entry
//! per build tool with a long description, plus a controls summary. The data
//! is structured (a tool list + i18n keys) so the later media encyclopedia
//! (images, videos) can grow from the same source without a rewrite.

use bevy::prelude::*;

use super::edit_hud::tool_key;
use super::widgets::{PANEL_BG, TEXT_BRIGHT, TEXT_DIM, button, text_bundle};
use crate::font::UiFont;
use crate::i18n::t;
use crate::state::{GameState, Tool};

/// Whether the help overlay is showing. Set true by [`HelpButton`] clicks on
/// either entry screen, false by the close button or Esc.
#[derive(Resource, Default)]
pub(super) struct HelpOpen(pub(super) bool);

/// Marks the "tool help" button on the main menu / level select.
#[derive(Component)]
pub(super) struct HelpButton;

/// The Esc-driven help close. Other Esc consumers on the level select
/// (`select::leave_to_menu`) order themselves `.before` this set so they read
/// [`HelpOpen`] before it is flipped — otherwise an Esc that closes the help
/// overlay could also fall through and leave the screen the same frame.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct HelpEscClose;

#[derive(Component)]
struct HelpRoot;

#[derive(Component)]
struct HelpCloseButton;

/// Tools documented in the overlay, in teaching order (Select last — it only
/// inspects). A plain list, not `Tool::ALL` (which is test-only).
const DOCUMENTED: [Tool; 8] = [
    Tool::Track,
    Tool::Switch,
    Tool::SignalBlock,
    Tool::SignalChain,
    Tool::Erase,
    Tool::Source,
    Tool::Sink,
    Tool::Select,
];

/// The i18n description key for a tool ("tool.track" → "tool.track.desc").
fn desc_key(tool: Tool) -> String {
    format!("{}.desc", tool_key(tool))
}

/// Every help-only i18n key, asserted present in both language tables by the
/// `crate::i18n` coverage test (the tool-name keys are already covered there
/// via `tool_key`).
#[cfg(test)]
pub(crate) const TOOL_HELP_KEYS: &[&str] = &[
    "help.button",
    "help.title",
    "help.intro",
    "help.controls",
    "help.close",
    "tool.select.desc",
    "tool.track.desc",
    "tool.switch.desc",
    "tool.signal_block.desc",
    "tool.signal_chain.desc",
    "tool.erase.desc",
    "tool.source.desc",
    "tool.sink.desc",
];

pub(super) struct EncyclopediaPlugin;

impl Plugin for EncyclopediaPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<HelpOpen>().add_systems(
            Update,
            (open_clicks, close_actions.in_set(HelpEscClose), sync_overlay)
                .run_if(in_state(GameState::MainMenu).or(in_state(GameState::LevelSelect))),
        );
        // Leaving either entry screen tears the overlay down with the screen
        // (sync_overlay no longer runs once the state's run-condition is off).
        app.add_systems(OnExit(GameState::MainMenu), teardown)
            .add_systems(OnExit(GameState::LevelSelect), teardown);
    }
}

fn teardown(
    mut commands: Commands,
    mut open: ResMut<HelpOpen>,
    roots: Query<Entity, With<HelpRoot>>,
) {
    open.0 = false;
    for e in &roots {
        commands.entity(e).despawn();
    }
}

fn open_clicks(
    interactions: Query<&Interaction, (Changed<Interaction>, With<HelpButton>)>,
    mut open: ResMut<HelpOpen>,
) {
    if interactions.iter().any(|i| *i == Interaction::Pressed) {
        open.0 = true;
    }
}

fn close_actions(
    interactions: Query<&Interaction, (Changed<Interaction>, With<HelpCloseButton>)>,
    keys: Res<ButtonInput<KeyCode>>,
    mut open: ResMut<HelpOpen>,
) {
    if !open.0 {
        return;
    }
    let by_button = interactions.iter().any(|i| *i == Interaction::Pressed);
    if by_button || keys.just_pressed(KeyCode::Escape) {
        open.0 = false;
    }
}

/// Spawns/despawns the overlay when [`HelpOpen`] changes.
fn sync_overlay(
    mut commands: Commands,
    open: Res<HelpOpen>,
    ui_font: Res<UiFont>,
    roots: Query<Entity, With<HelpRoot>>,
) {
    if !open.is_changed() {
        return;
    }
    for e in &roots {
        commands.entity(e).despawn();
    }
    if !open.0 {
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
            // Absorb clicks so they do not fall through to the screen behind.
            Interaction::default(),
            HelpRoot,
        ))
        .with_children(|root| {
            root.spawn((
                Node {
                    flex_direction: FlexDirection::Column,
                    max_width: Val::Px(820.0),
                    padding: UiRect::all(Val::Px(18.0)),
                    row_gap: Val::Px(7.0),
                    ..default()
                },
                BackgroundColor(PANEL_BG),
            ))
            .with_children(|panel| {
                panel.spawn(text_bundle(&font, t("help.title"), 28.0, TEXT_BRIGHT));
                panel.spawn(text_bundle(&font, t("help.intro"), 13.0, TEXT_DIM));
                panel.spawn((
                    text_bundle(&font, t("help.controls"), 13.0, TEXT_DIM),
                    Node {
                        margin: UiRect::bottom(Val::Px(8.0)),
                        ..default()
                    },
                ));
                for tool in DOCUMENTED {
                    panel.spawn(text_bundle(
                        &font,
                        format!("» {}", t(tool_key(tool))),
                        16.0,
                        TEXT_BRIGHT,
                    ));
                    panel.spawn((
                        text_bundle(&font, t(&desc_key(tool)), 12.0, TEXT_DIM),
                        Node {
                            margin: UiRect::bottom(Val::Px(4.0)),
                            ..default()
                        },
                    ));
                }
                button(panel, &font, &t("help.close"), PANEL_BG, HelpCloseButton);
            });
        });
}
