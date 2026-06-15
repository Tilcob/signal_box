//! Timetable panel (bottom left): read-only rows in campaign levels, an
//! editable per-train row editor in the sandbox.

use bevy::prelude::*;
use stellwerk_sim::level::{Level, ScheduleEntry};
use stellwerk_sim::units::{Len, SinkId, SourceId, Speed, Tick, TrainClass, TrainId};

use super::numeric_field::{NumericFieldCommit, numeric_field};
use super::widgets::{TEXT_BRIGHT, TEXT_DIM, small_button, text_bundle};
use crate::editor::{EditOp, do_op};
use crate::font::UiFont;
use crate::i18n::{station_label, t};
use crate::state::{ActiveLevel, EditNotice, Editor, GameState};

/// How long a refused-action notice stays on the Edit HUD.
const NOTICE_SECS: f32 = 4.0;

// Editor-edge clamps for the numeric fields. Not balance — just "no nonsense":
// ticks are non-negative, lengths positive, and speed must stay below the
// shortest edge (anti-tunneling, `MAX_SPEED_EXCLUSIVE`).
const TICK_MAX: i64 = 99_999;
const LEN_MIN: i64 = 1;
const LEN_MAX: i64 = 9_999;
const SPEED_MIN: i64 = 1;
const SPEED_MAX: i64 = stellwerk_sim::units::segment_lengths::MAX_SPEED_EXCLUSIVE - 1;

/// Which numeric column of a schedule row a [`NumericField`] edits.
#[derive(Clone, Copy)]
enum SchedFieldKind {
    Depart,
    Due,
    Speed,
    Length,
}

/// Marker on a schedule numeric field, mapping commits back to the row/column.
#[derive(Component, Clone, Copy)]
struct SchedField {
    row: usize,
    kind: SchedFieldKind,
}

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
}

pub(super) struct SchedulePanelPlugin;

impl Plugin for SchedulePanelPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                rebuild_schedule_panel.run_if(resource_exists_and_changed::<ActiveLevel>),
                schedule_clicks,
                schedule_field_commits,
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
            .map(|s| station_label(&s.label))
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
                        format!("{} {}", t("common.train"), entry.train.0),
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
                    // depart/due/speed/length are now typed, not cycled
                    // (restfeature 03). Prefix label + focusable numeric field.
                    let field = |r: &mut ChildSpawnerCommands,
                                     label: &str,
                                     value: i64,
                                     min: i64,
                                     max: i64,
                                     kind: SchedFieldKind| {
                        r.spawn(text_bundle(&font, label.to_string(), 12.0, TEXT_DIM));
                        numeric_field(r, &font, value, min, max, SchedField { row, kind });
                    };
                    field(r, "ab", entry.depart.0 as i64, 0, TICK_MAX, SchedFieldKind::Depart);
                    field(r, "soll", entry.due.0 as i64, 0, TICK_MAX, SchedFieldKind::Due);
                    field(r, "v", entry.speed.0, SPEED_MIN, SPEED_MAX, SchedFieldKind::Speed);
                    field(r, "L", entry.length.0, LEN_MIN, LEN_MAX, SchedFieldKind::Length);
                    small_button(r, &font, "×", SchedAction::Remove(row));
                });
        }
    });
}

/// Builds a [`EditOp::ScheduleEdit`] for `row`: clones the current entry as
/// `before`, applies `f` to a copy for `after`. `None` if the row is gone.
fn edit_row(level: &Level, row: usize, f: impl FnOnce(&mut ScheduleEntry)) -> Option<EditOp> {
    let before = level.schedule.get(row)?.clone();
    let mut after = before.clone();
    f(&mut after);
    Some(EditOp::ScheduleEdit { row, before, after })
}

fn schedule_clicks(
    mut interactions: Query<(&Interaction, &SchedAction), Changed<Interaction>>,
    active: Option<ResMut<ActiveLevel>>,
    mut editor: ResMut<Editor>,
    mut notice: ResMut<EditNotice>,
) {
    let Some(mut active) = active else { return };
    if !active.sandbox {
        return;
    }
    for (interaction, action) in &mut interactions {
        if *interaction != Interaction::Pressed {
            continue;
        }
        // Unknown current value (e.g. imported level) → start at the first
        // entry instead of silently skipping it.
        let cycle = |current: u32, list: &[u32]| -> u32 {
            let pos = list
                .iter()
                .position(|&v| v == current)
                .map_or(0, |p| (p + 1) % list.len());
            list[pos]
        };
        // Each edit becomes one invertible op on the shared undo stack
        // (restfeature 02); the level is mutated only via `do_op` below.
        let op = match *action {
            SchedAction::Add => {
                let (Some(source), Some(sink)) =
                    (active.level.sources.first(), active.level.sinks.first())
                else {
                    // Don't swallow the click: tell the player WHAT is missing.
                    let key = match (
                        active.level.sources.is_empty(),
                        active.level.sinks.is_empty(),
                    ) {
                        (true, true) => "schedule.need_source_and_sink",
                        (false, true) => "schedule.need_sink",
                        (true, false) => "schedule.need_source",
                        (false, false) => unreachable!("matched only when one is None"),
                    };
                    notice.0 = Some((t(key), Timer::from_seconds(NOTICE_SECS, TimerMode::Once)));
                    continue;
                };
                let train = TrainId(
                    active
                        .level
                        .schedule
                        .iter()
                        .map(|e| e.train.0)
                        .max()
                        .map_or(0, |m| m + 1),
                );
                let depart = Tick(active.level.schedule.last().map_or(0, |e| e.depart.0 + 10));
                let entry = ScheduleEntry {
                    train,
                    class: TrainClass(0),
                    length: Len(800),
                    speed: Speed(100),
                    source: source.id,
                    sink: sink.id,
                    depart,
                    due: Tick(depart.0 + 80),
                };
                Some(EditOp::ScheduleInsert {
                    row: active.level.schedule.len(),
                    entry,
                })
            }
            SchedAction::Remove(row) => {
                active
                    .level
                    .schedule
                    .get(row)
                    .map(|e| EditOp::ScheduleRemove {
                        row,
                        entry: e.clone(),
                    })
            }
            SchedAction::CycleSource(row) => {
                let ids: Vec<u32> = active.level.sources.iter().map(|s| s.id.0).collect();
                edit_row(&active.level, row, |e| {
                    if !ids.is_empty() {
                        e.source = SourceId(cycle(e.source.0, &ids));
                    }
                })
            }
            SchedAction::CycleSink(row) => {
                let ids: Vec<u32> = active.level.sinks.iter().map(|s| s.id.0).collect();
                edit_row(&active.level, row, |e| {
                    if !ids.is_empty() {
                        e.sink = SinkId(cycle(e.sink.0, &ids));
                    }
                })
            }
            SchedAction::CycleClass(row) => {
                edit_row(&active.level, row, |e| e.class = TrainClass((e.class.0 + 1) % 2))
            }
        };
        if let Some(op) = op {
            do_op(&mut editor, &mut active.level, op);
        }
    }
}

/// Applies committed numeric-field edits (depart/due/speed/length) as one
/// `ScheduleEdit` op each — the typed counterpart of the cycle clicks.
fn schedule_field_commits(
    mut commits: MessageReader<NumericFieldCommit>,
    fields: Query<&SchedField>,
    active: Option<ResMut<ActiveLevel>>,
    mut editor: ResMut<Editor>,
) {
    let Some(mut active) = active else { return };
    if !active.sandbox {
        return;
    }
    for commit in commits.read() {
        let Ok(&SchedField { row, kind }) = fields.get(commit.field) else {
            continue;
        };
        let value = commit.value;
        let op = edit_row(&active.level, row, |e| match kind {
            SchedFieldKind::Depart => e.depart = Tick(value as u64),
            SchedFieldKind::Due => e.due = Tick(value as u64),
            SchedFieldKind::Speed => e.speed = Speed(value),
            SchedFieldKind::Length => e.length = Len(value),
        });
        if let Some(op) = op {
            do_op(&mut editor, &mut active.level, op);
        }
    }
}
