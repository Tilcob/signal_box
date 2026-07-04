//! One-time onboarding hints (GDD §13: "kontextuelle Ersthilfen … einmalige
//! Hinweise, keine Tutorial-Zwangsführung"). A hint is enabled purely by
//! authoring a `hint.<level_id>` i18n string — this module is the mechanism,
//! the texts are content. It shows once on first entering a level's editor and
//! records the id in `Progress::seen_hints`. A persistent "?" button (top-right,
//! under START) re-opens it any time after, so a dismissed hint is never lost.

use bevy::prelude::*;
use bevy::text::Font;

use super::widgets::{BUTTON_BG, PANEL_BG, TEXT_BRIGHT, button, despawn_all, text_bundle};
use crate::font::UiFont;
use crate::i18n::{t, t_or};
use crate::levels::Progress;
use crate::state::{ActiveLevel, GameState};

/// Shared marker on every hint entity (panel and recall button), so leaving the
/// editor cleans them all in one `despawn_all`.
#[derive(Component)]
struct HintUi;
/// The dismissable hint panel root (its own marker so dismiss removes only it,
/// leaving the recall button).
#[derive(Component)]
struct HintRoot;
#[derive(Component)]
struct HintDismiss;
/// The persistent "?" button that re-opens the level hint after dismissal.
#[derive(Component)]
struct HintRecall;

pub(super) struct HintsPlugin;

impl Plugin for HintsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameState::Edit), enter)
            .add_systems(OnExit(GameState::Edit), despawn_all::<HintUi>)
            .add_systems(
                Update,
                (dismiss_click, reopen_click).run_if(in_state(GameState::Edit)),
            );
    }
}

/// On entering a campaign level's editor: if it has an authored hint, place the
/// persistent "?" recall button, and on the first visit show the hint once and
/// mark it seen so it does not auto-open again.
fn enter(
    mut commands: Commands,
    ui_font: Res<UiFont>,
    active: Option<Res<ActiveLevel>>,
    mut progress: ResMut<Progress>,
) {
    let Some(active) = active else { return };
    if active.sandbox {
        return;
    }
    let text = t_or(&format!("hint.{}", active.id), "");
    if text.is_empty() {
        return; // no hint authored for this level → no recall button either
    }
    let font = ui_font.0.clone();

    // Persistent recall button, just below the top-right START button.
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                right: Val::Px(10.0),
                top: Val::Px(52.0),
                ..default()
            },
            Interaction::default(),
            HintUi,
        ))
        .with_children(|c| {
            button(c, &font, "?", BUTTON_BG, HintRecall);
        });

    // First visit only: show the hint and remember it.
    if !progress.seen_hints.contains(&active.id) {
        progress.seen_hints.insert(active.id.clone());
        progress.save();
        spawn_hint_panel(&mut commands, &font, text);
    }
}

/// Spawns the centred hint panel (text + dismiss button). Shared by the first
/// auto-show and the "?" recall.
fn spawn_hint_panel(commands: &mut Commands, font: &Handle<Font>, text: String) {
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
            HintUi,
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
                panel.spawn(text_bundle(font, text, 16.0, TEXT_BRIGHT));
                button(panel, font, &t("hint.dismiss"), BUTTON_BG, HintDismiss);
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

/// The "?" button re-opens the hint — unless it is already open (no stacking).
fn reopen_click(
    interactions: Query<&Interaction, (Changed<Interaction>, With<HintRecall>)>,
    open: Query<(), With<HintRoot>>,
    mut commands: Commands,
    ui_font: Res<UiFont>,
    active: Option<Res<ActiveLevel>>,
) {
    if !interactions.iter().any(|i| *i == Interaction::Pressed) || !open.is_empty() {
        return;
    }
    let Some(active) = active else { return };
    let text = t_or(&format!("hint.{}", active.id), "");
    if !text.is_empty() {
        spawn_hint_panel(&mut commands, &ui_font.0, text);
    }
}
