//! Timetable panel (bottom left): read-only rows in campaign levels, an
//! editable per-train row editor in the sandbox.

use bevy::prelude::*;
use stellwerk_sim::level::ScheduleEntry;
use stellwerk_sim::units::{Len, SinkId, Speed, Tick, TrainClass, TrainId};

use super::widgets::{TEXT_BRIGHT, TEXT_DIM, small_button, text_bundle};
use crate::font::UiFont;
use crate::i18n::t;
use crate::state::{ActiveLevel, GameState};

/// Root node, spawned by the edit HUD (it owns the Edit screen layout).
#[derive(Component)]
pub(super) struct SchedulePanelRoot;

#[derive(Component, Clone, Copy)]
pub(super) enum SchedAction {
    Add,
    Remove(usize),
    CycleSource(usize),
    CycleSink(usize),
    CycleClass(usize),
    BumpDepart(usize),
    BumpDue(usize),
    CycleSpeed(usize),
    CycleLength(usize),
}

pub(super) struct SchedulePanelPlugin;

impl Plugin for SchedulePanelPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                rebuild_schedule_panel.run_if(resource_exists_and_changed::<ActiveLevel>),
                schedule_clicks,
            )
                .run_if(in_state(GameState::Edit)),
        );
    }
}

pub(super) fn rebuild_schedule_panel(
    mut commands: Commands,
    ui_font: Res<UiFont>,
    roots: Query<Entity, With<SchedulePanelRoot>>,
    active: Option<Res<ActiveLevel>>,
) {
    let Ok(root) = roots.single() else { return };
    let Some(active) = active else { return };
    commands.entity(root).despawn_children();
    let font = ui_font.0.clone();
    let level = active.level.clone();
    let sandbox = active.sandbox;
    let sink_label = |level: &stellwerk_sim::Level, id: SinkId| {
        level
            .sinks
            .iter()
            .find(|s| s.id == id)
            .map(|s| s.label.clone())
            .unwrap_or_else(|| format!("Z{}", id.0))
    };
    commands.entity(root).with_children(|panel| {
        panel.spawn(text_bundle(&font, t("schedule.title"), 15.0, TEXT_BRIGHT));
        if !sandbox {
            for entry in &level.schedule {
                panel.spawn((
                    text_bundle(
                        &font,
                        format!(
                            "{}{} · {}{} · Q{} → {} · {}{} · {}{} · {}{} · {}{}",
                            t("schedule.train"),
                            entry.train.0,
                            t("schedule.class"),
                            entry.class.0,
                            entry.source.0,
                            sink_label(&level, entry.sink),
                            t("schedule.depart"),
                            entry.depart.0,
                            t("schedule.due"),
                            entry.due.0,
                            t("schedule.speed"),
                            entry.speed.0,
                            t("schedule.length"),
                            entry.length.0,
                        ),
                        13.0,
                        TEXT_DIM,
                    ),
                    Node {
                        margin: UiRect::top(Val::Px(4.0)),
                        ..default()
                    },
                ));
            }
            return;
        }
        for (row, entry) in level.schedule.iter().enumerate() {
            panel
                .spawn(Node {
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    ..default()
                })
                .with_children(|r| {
                    r.spawn(text_bundle(
                        &font,
                        format!("Zug {}", entry.train.0),
                        13.0,
                        TEXT_DIM,
                    ));
                    small_button(
                        r,
                        &font,
                        &format!("Q{}", entry.source.0),
                        SchedAction::CycleSource(row),
                    );
                    small_button(
                        r,
                        &font,
                        &format!("→{}", sink_label(&level, entry.sink)),
                        SchedAction::CycleSink(row),
                    );
                    small_button(
                        r,
                        &font,
                        &format!("K{}", entry.class.0),
                        SchedAction::CycleClass(row),
                    );
                    small_button(
                        r,
                        &font,
                        &format!("ab {}", entry.depart.0),
                        SchedAction::BumpDepart(row),
                    );
                    small_button(
                        r,
                        &font,
                        &format!("soll {}", entry.due.0),
                        SchedAction::BumpDue(row),
                    );
                    small_button(
                        r,
                        &font,
                        &format!("v{}", entry.speed.0),
                        SchedAction::CycleSpeed(row),
                    );
                    small_button(
                        r,
                        &font,
                        &format!("L{}", entry.length.0),
                        SchedAction::CycleLength(row),
                    );
                    small_button(r, &font, "×", SchedAction::Remove(row));
                });
        }
    });
}

fn schedule_clicks(
    mut interactions: Query<(&Interaction, &SchedAction), Changed<Interaction>>,
    active: Option<ResMut<ActiveLevel>>,
) {
    let Some(mut active) = active else { return };
    if !active.sandbox {
        return;
    }
    for (interaction, action) in &mut interactions {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let level = &mut active.level;
        // Unknown current value (e.g. imported level) → start at the first
        // entry instead of silently skipping it.
        let cycle = |current: u32, list: &[u32]| -> u32 {
            let pos = list
                .iter()
                .position(|&v| v == current)
                .map_or(0, |p| (p + 1) % list.len());
            list[pos]
        };
        match *action {
            SchedAction::Add => {
                let (Some(source), Some(sink)) = (level.sources.first(), level.sinks.first())
                else {
                    continue; // needs at least one source and sink
                };
                let train = TrainId(
                    level
                        .schedule
                        .iter()
                        .map(|e| e.train.0)
                        .max()
                        .map_or(0, |m| m + 1),
                );
                let depart = Tick(level.schedule.last().map_or(0, |e| e.depart.0 + 10));
                level.schedule.push(ScheduleEntry {
                    train,
                    class: TrainClass(0),
                    length: Len(800),
                    speed: Speed(100),
                    source: source.id,
                    sink: sink.id,
                    depart,
                    due: Tick(depart.0 + 80),
                });
            }
            SchedAction::Remove(row) => {
                if row < level.schedule.len() {
                    level.schedule.remove(row);
                }
            }
            SchedAction::CycleSource(row) => {
                let ids: Vec<u32> = level.sources.iter().map(|s| s.id.0).collect();
                if let (Some(entry), false) = (level.schedule.get_mut(row), ids.is_empty()) {
                    entry.source = stellwerk_sim::units::SourceId(cycle(entry.source.0, &ids));
                }
            }
            SchedAction::CycleSink(row) => {
                let ids: Vec<u32> = level.sinks.iter().map(|s| s.id.0).collect();
                if let (Some(entry), false) = (level.schedule.get_mut(row), ids.is_empty()) {
                    entry.sink = SinkId(cycle(entry.sink.0, &ids));
                }
            }
            SchedAction::CycleClass(row) => {
                if let Some(entry) = level.schedule.get_mut(row) {
                    entry.class = TrainClass((entry.class.0 + 1) % 2);
                }
            }
            SchedAction::BumpDepart(row) => {
                if let Some(entry) = level.schedule.get_mut(row) {
                    entry.depart = Tick((entry.depart.0 + 10) % 200);
                    entry.due = Tick(entry.due.0.max(entry.depart.0 + 40));
                }
            }
            SchedAction::BumpDue(row) => {
                if let Some(entry) = level.schedule.get_mut(row) {
                    entry.due = Tick(entry.due.0 + 20);
                    if entry.due.0 > entry.depart.0 + 400 {
                        entry.due = Tick(entry.depart.0 + 40);
                    }
                }
            }
            SchedAction::CycleSpeed(row) => {
                if let Some(entry) = level.schedule.get_mut(row) {
                    entry.speed = Speed(cycle(entry.speed.0 as u32, &[60, 100, 150, 240]) as i64);
                }
            }
            SchedAction::CycleLength(row) => {
                if let Some(entry) = level.schedule.get_mut(row) {
                    entry.length = Len(cycle(entry.length.0 as u32, &[800, 1400, 1800]) as i64);
                }
            }
        }
    }
}
