//! Run mode: drives the deterministic sim with a fixed-tick accumulator
//! (nominal 10 ticks/s × speed), interpolates train heads for rendering and
//! routes the outcome into the Result state (plan M1 §2).

use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use std::collections::BTreeMap;
use stellwerk_sim::sim::SimEvent;
use stellwerk_sim::train::Train;
use stellwerk_sim::units::{Tick, TrainId};
use stellwerk_sim::{Layout, Outcome, Sim, TrackGraph};

use crate::board::point_world;
use crate::camera::{MainCamera, cursor_world};
use crate::i18n::{station_label, t};
use crate::levels::Progress;
use crate::state::{ActiveLevel, Editor, GameState, LastOutcome, not_paused};

pub const TICKS_PER_SECOND: f32 = 10.0;
/// Hard cap so a hung run cannot freeze the app (sim has its own caps too).
const MAX_STEPS_PER_FRAME: u32 = 64;

#[derive(Resource)]
pub struct RunCtl {
    pub sim: Sim,
    /// Player layout of this run (for signal lamps and the autosave).
    pub layout: Layout,
    /// 0 = pause, otherwise speed factor (1/4/16).
    pub speed: u32,
    last_speed: u32,
    /// Fraction of the next tick already elapsed (also the lerp factor).
    acc: f32,
    heads_prev: BTreeMap<TrainId, Vec2>,
    heads_curr: BTreeMap<TrainId, Vec2>,
}

impl RunCtl {
    pub fn interpolated_head(&self, id: TrainId) -> Vec2 {
        let curr = self.heads_curr.get(&id).copied().unwrap_or_default();
        let prev = self.heads_prev.get(&id).copied().unwrap_or(curr);
        prev.lerp(curr, self.acc.clamp(0.0, 1.0))
    }
}

fn head_world(graph: &TrackGraph, train: &Train) -> Vec2 {
    let edge = graph.edge(train.head_edge());
    let a = point_world(graph.node(edge.from).point);
    let b = point_world(graph.node(edge.to).point);
    a.lerp(b, train.head_dist.0 as f32 / edge.len.0 as f32)
}

fn heads(sim: &Sim) -> BTreeMap<TrainId, Vec2> {
    sim.trains()
        .iter()
        .map(|t| (t.id, head_world(sim.graph(), t)))
        .collect()
}

/// HUD line for a clicked train.
#[derive(Resource, Default)]
pub struct TrainInfo(pub Option<String>);

pub struct RunPlugin;

impl Plugin for RunPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<TrainInfo>()
            .add_systems(OnEnter(GameState::Run), start_run)
            .add_systems(
                Update,
                (
                    // Frozen while the pause menu is open; `toggle_pause` stays
                    // ungated so Esc can still close it.
                    speed_input.run_if(not_paused),
                    tick.run_if(not_paused),
                    click_train.run_if(not_paused),
                    crate::ui::pause::toggle_pause,
                )
                    .run_if(in_state(GameState::Run)),
            )
            .add_systems(OnEnter(GameState::Edit), cleanup)
            .add_systems(OnEnter(GameState::LevelSelect), cleanup);
    }
}

fn cleanup(mut commands: Commands, mut info: ResMut<TrainInfo>) {
    commands.remove_resource::<RunCtl>();
    info.0 = None;
}

fn start_run(
    mut commands: Commands,
    active: Option<Res<ActiveLevel>>,
    editor: Res<Editor>,
    mut progress: ResMut<Progress>,
    mut next: ResMut<NextState<GameState>>,
) {
    let Some(active) = active else {
        next.set(GameState::LevelSelect);
        return;
    };
    // Autosave the build the moment the run starts (plan M1 §3).
    progress.entry(&active.id).layout = editor.layout.clone();
    progress.save();

    match Sim::new(&active.level, &editor.layout) {
        Ok(sim) => {
            let curr = heads(&sim);
            commands.insert_resource(RunCtl {
                sim,
                layout: editor.layout.clone(),
                speed: 1,
                last_speed: 1,
                acc: 0.0,
                heads_prev: curr.clone(),
                heads_curr: curr,
            });
        }
        Err(errors) => {
            // The start gate should prevent this; never crash regardless.
            warn!("run refused, layout invalid: {errors:?}");
            next.set(GameState::Edit);
        }
    }
}

fn speed_input(keys: Res<ButtonInput<KeyCode>>, ctl: Option<ResMut<RunCtl>>) {
    let Some(mut ctl) = ctl else { return };
    if keys.just_pressed(KeyCode::Space) {
        if ctl.speed == 0 {
            ctl.speed = ctl.last_speed.max(1);
        } else {
            ctl.last_speed = ctl.speed;
            ctl.speed = 0;
        }
    }
    if keys.just_pressed(KeyCode::Digit1) {
        ctl.speed = 1;
    }
    if keys.just_pressed(KeyCode::Digit2) {
        ctl.speed = 4;
    }
    if keys.just_pressed(KeyCode::Digit3) {
        ctl.speed = 16;
    }
}

#[allow(clippy::too_many_arguments)]
fn tick(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    ctl: Option<ResMut<RunCtl>>,
    active: Option<Res<ActiveLevel>>,
    mut progress: ResMut<Progress>,
    mut commands: Commands,
    mut next: ResMut<NextState<GameState>>,
) {
    let (Some(mut ctl), Some(active)) = (ctl, active) else {
        return;
    };

    let mut steps = 0u32;
    if ctl.speed > 0 {
        ctl.acc += time.delta_secs() * TICKS_PER_SECOND * ctl.speed as f32;
        steps = (ctl.acc as u32).min(MAX_STEPS_PER_FRAME);
        ctl.acc -= steps as f32;
        ctl.acc = ctl.acc.min(1.0);
    } else if keys.just_pressed(KeyCode::KeyT) {
        steps = 1; // single tick while paused
        ctl.acc = 0.0;
    }

    for _ in 0..steps {
        ctl.heads_prev = std::mem::take(&mut ctl.heads_curr);
        let events: Vec<SimEvent> = ctl.sim.step().to_vec();
        ctl.heads_curr = heads(&ctl.sim);
        for event in events {
            match event {
                SimEvent::TrainSpawned(_) => {
                    commands.trigger(crate::audio::SfxKind::Rail);
                }
                SimEvent::SignalBlocked { .. } => {
                    commands.trigger(crate::audio::SfxKind::Signal);
                }
                SimEvent::RunEnded(outcome) => {
                    finish(&outcome, &active, &mut progress, &mut commands, &mut next);
                    return;
                }
                SimEvent::TrainArrived { .. } => {
                    commands.trigger(crate::audio::SfxKind::TrainHorn);
                }
            }
        }
        // Safety net: a run that never ends within the sim cap.
        if ctl.sim.now() >= Tick(100_000) {
            let outcome = ctl.sim.run(Tick(100_000));
            finish(&outcome, &active, &mut progress, &mut commands, &mut next);
            return;
        }
    }
}

fn finish(
    outcome: &Outcome,
    active: &ActiveLevel,
    progress: &mut Progress,
    commands: &mut Commands,
    next: &mut NextState<GameState>,
) {
    if let Outcome::Success { score } = outcome {
        progress.entry(&active.id).record(score);
        progress.save();
    }
    commands.insert_resource(LastOutcome(outcome.clone()));
    next.set(GameState::Result);
}

/// Clicking near a train head shows its details in the HUD (plan M1:
/// "klickbarer Zug zeigt Ziel + Wartezustand").
fn click_train(
    buttons: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    cameras: Query<(&Camera, &GlobalTransform), With<MainCamera>>,
    ctl: Option<Res<RunCtl>>,
    active: Option<Res<ActiveLevel>>,
    mut info: ResMut<TrainInfo>,
) {
    if !buttons.just_pressed(MouseButton::Left) {
        return;
    }
    let (Some(ctl), Some(active)) = (ctl, active) else {
        return;
    };
    let Some(cursor) = cursor_world(&windows, &cameras) else {
        return;
    };
    info.0 = None;
    for train in ctl.sim.trains() {
        let head = ctl.interpolated_head(train.id);
        if head.distance(cursor) < 30.0 {
            let sink = active
                .level
                .sinks
                .iter()
                .find(|s| s.id == train.sink)
                .map(|s| station_label(&s.label))
                .unwrap_or_else(|| format!("{} {}", t("common.sink"), train.sink.0));
            let waiting = match train.waiting_since {
                Some(since) => format!(" · {} {}", t("run.train_waiting"), since.0),
                None => String::new(),
            };
            info.0 = Some(format!(
                "{} {} → {sink} · {} {}{waiting}",
                t("common.train"),
                train.id.0,
                t("run.train_due"),
                train.due.0
            ));
            return;
        }
    }
}
