//! Run HUD: tick counter, speed/pause line and the clicked-train info.

use bevy::prelude::*;

use super::widgets::{TEXT_BRIGHT, TEXT_DIM, despawn_all, text_bundle};
use crate::font::UiFont;
use crate::i18n::{level_name, t};
use crate::run::{RunCtl, TrainInfo};
use crate::state::{ActiveLevel, GameState};

#[derive(Component)]
struct UiRun;
#[derive(Component)]
struct SpeedText;
#[derive(Component)]
struct InfoText;

pub(super) struct RunHudPlugin;

impl Plugin for RunHudPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameState::Run), spawn_run_hud)
            // Cleanup on ENTERING Edit/LevelSelect, not on leaving Result:
            // Esc skips Result entirely (Run → Edit → LevelSelect), and an
            // OnExit(Result)-only despawn leaks the HUD as a ghost overlay —
            // re-entering Run then even stacks a second copy. Run → Result
            // has no despawn, so the HUD stays visible behind the overlay.
            .add_systems(OnEnter(GameState::Edit), despawn_all::<UiRun>)
            .add_systems(OnEnter(GameState::LevelSelect), despawn_all::<UiRun>)
            .add_systems(
                Update,
                update_run_texts.run_if(in_state(GameState::Run).or(in_state(GameState::Result))),
            );
    }
}

fn spawn_run_hud(
    mut commands: Commands,
    ui_font: Res<UiFont>,
    active: Option<Res<ActiveLevel>>,
) {
    let font = ui_font.0.clone();
    let name = active
        .map(|a| level_name(&a.id, &a.level.name))
        .unwrap_or_default();
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(10.0),
                top: Val::Px(8.0),
                flex_direction: FlexDirection::Column,
                ..default()
            },
            UiRun,
        ))
        .with_children(|c| {
            c.spawn(text_bundle(&font, name, 22.0, TEXT_BRIGHT));
            c.spawn((
                text_bundle(&font, String::new(), 16.0, TEXT_BRIGHT),
                SpeedText,
            ));
            c.spawn(text_bundle(&font, t("run.hints"), 13.0, TEXT_DIM));
            c.spawn((text_bundle(&font, String::new(), 14.0, TEXT_DIM), InfoText));
        });
}

fn update_run_texts(
    ctl: Option<Res<RunCtl>>,
    info: Res<TrainInfo>,
    mut speed_texts: Query<&mut Text, (With<SpeedText>, Without<InfoText>)>,
    mut info_texts: Query<&mut Text, (With<InfoText>, Without<SpeedText>)>,
    // Reused across frames: format into it instead of allocating a fresh
    // String every frame (this system runs every frame during a run).
    mut buf: Local<String>,
) {
    use std::fmt::Write as _;
    let Some(ctl) = ctl else { return };
    if let Ok(mut text) = speed_texts.single_mut() {
        buf.clear();
        if ctl.speed == 0 {
            let _ = write!(buf, "Tick {}   {}", ctl.sim.now().0, t("run.paused"));
        } else {
            let _ = write!(buf, "Tick {}   ×{}", ctl.sim.now().0, ctl.speed);
        }
        // Same change-guard as `set_text`, but comparing against the buffer so
        // the assignment (and text relayout) only happens on a real change.
        if text.0 != *buf {
            text.0.clear();
            text.0.push_str(&buf);
        }
    }
    if let Ok(mut text) = info_texts.single_mut() {
        let info = info.0.as_deref().unwrap_or_default();
        if text.0 != info {
            text.0.clear();
            text.0.push_str(info);
        }
    }
}
