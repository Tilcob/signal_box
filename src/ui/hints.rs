//! One-time onboarding hints (GDD §13: "kontextuelle Ersthilfen … einmalige
//! Hinweise, keine Tutorial-Zwangsführung"). A hint is enabled purely by
//! authoring a `hint.<level_id>` i18n string — this module is the mechanism,
//! the texts are content. It shows once on first entering a level's editor,
//! records the id in `Progress::seen_hints`, and never returns.

use bevy::prelude::*;

use super::widgets::{BUTTON_BG, PANEL_BG, TEXT_BRIGHT, button, text_bundle};
use crate::font::UiFont;
use crate::i18n::{t, t_or};
use crate::levels::Progress;
use crate::state::{ActiveLevel, GameState};

#[derive(Component)]
struct HintRoot;
#[derive(Component)]
struct HintDismiss;

pub(super) struct HintsPlugin;

impl Plugin for HintsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameState::Edit), show_hint)
            .add_systems(OnExit(GameState::Edit), teardown)
            .add_systems(Update, dismiss_click.run_if(in_state(GameState::Edit)));
    }
}

/// On entering a campaign level's editor, show its hint once (if authored and
/// not yet seen) and mark it seen so it never reappears.
fn show_hint(
    mut commands: Commands,
    ui_font: Res<UiFont>,
    active: Option<Res<ActiveLevel>>,
    mut progress: ResMut<Progress>,
) {
    let Some(active) = active else { return };
    if active.sandbox || progress.seen_hints.contains(&active.id) {
        return;
    }
    let text = t_or(&format!("hint.{}", active.id), "");
    if text.is_empty() {
        return; // no hint authored for this level
    }
    progress.seen_hints.insert(active.id.clone());
    progress.save();

    let font = ui_font.0.clone();
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(70.0),
                width: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                ..default()
            },
            HintRoot,
        ))
        .with_children(|root| {
            root.spawn((
                Node {
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    width: Val::Px(520.0),
                    padding: UiRect::all(Val::Px(16.0)),
                    row_gap: Val::Px(10.0),
                    ..default()
                },
                BackgroundColor(PANEL_BG),
            ))
            .with_children(|panel| {
                panel.spawn(text_bundle(&font, text, 16.0, TEXT_BRIGHT));
                button(panel, &font, &t("hint.dismiss"), BUTTON_BG, HintDismiss);
            });
        });
}

fn dismiss_click(
    interactions: Query<&Interaction, (Changed<Interaction>, With<HintDismiss>)>,
    mut commands: Commands,
    roots: Query<Entity, With<HintRoot>>,
) {
    if interactions.iter().any(|i| *i == Interaction::Pressed) {
        for e in &roots {
            commands.entity(e).despawn();
        }
    }
}

fn teardown(mut commands: Commands, roots: Query<Entity, With<HintRoot>>) {
    for e in &roots {
        commands.entity(e).despawn();
    }
}
