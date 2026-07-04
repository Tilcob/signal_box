//! One-time onboarding hints (GDD §13: "kontextuelle Ersthilfen … einmalige
//! Hinweise, keine Tutorial-Zwangsführung"). A hint is enabled purely by
//! authoring a `hint.<level_id>` i18n string — this module is the mechanism,
//! the texts are content. It shows once on first entering a level's editor and
//! records the id in `Progress::seen_hints`. The persistent "?" recall button
//! (spawned by `edit_hud::start`, left of START) re-opens it any time after, so
//! a dismissed hint is never lost.

use bevy::prelude::*;

use super::widgets::{BUTTON_BG, PANEL_BG, TEXT_BRIGHT, button, despawn_all, text_bundle};
use crate::font::UiFont;
use crate::i18n::{t, t_or};
use crate::levels::Progress;
use crate::state::{ActiveLevel, GameState};

/// The dismissable hint panel root.
#[derive(Component)]
struct HintRoot;
#[derive(Component)]
struct HintDismiss;
/// The persistent "?" recall button. Placed by `edit_hud::start` (so it sits in
/// the top-right row next to START); this module only handles its clicks.
#[derive(Component)]
pub(crate) struct HintRecall;

pub(super) struct HintsPlugin;

impl Plugin for HintsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameState::Edit), show_hint)
            .add_systems(OnExit(GameState::Edit), despawn_all::<HintRoot>)
            .add_systems(
                Update,
                (dismiss_click, reopen_click).run_if(in_state(GameState::Edit)),
            );
    }
}

/// Whether the given level has an authored hint (and so should carry a recall
/// button). Sandbox levels never do.
pub(crate) fn has_hint(active: &ActiveLevel) -> bool {
    !active.sandbox && !t_or(&format!("hint.{}", active.id), "").is_empty()
}

/// On first entering a campaign level's editor, show its hint once and mark it
/// seen so it does not auto-open again.
fn show_hint(
    mut commands: Commands,
    ui_font: Res<UiFont>,
    active: Option<Res<ActiveLevel>>,
    mut progress: ResMut<Progress>,
) {
    let Some(active) = active else { return };
    if !has_hint(&active) || progress.seen_hints.contains(&active.id) {
        return;
    }
    let text = t_or(&format!("hint.{}", active.id), "");
    progress.seen_hints.insert(active.id.clone());
    progress.save();
    spawn_hint_panel(&mut commands, &ui_font.0, text);
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
