//! Timetable panel (bottom left): read-only rows in campaign levels, an
//! editable per-train row editor in the sandbox.

use bevy::input::mouse::{MouseScrollUnit, MouseWheel};
use bevy::prelude::*;
use bevy::ui::ScrollPosition;
use stellwerk_sim::level::{Level, PlatformStop, ScheduleEntry};
use stellwerk_sim::units::{Len, PlatformId, SinkId, SourceId, Speed, Tick, TrainClass, TrainId};

use super::numeric_field::{NumericFieldCommit, numeric_field};
use super::widgets::{TEXT_BRIGHT, TEXT_DIM, small_button, text_bundle};
use crate::console::ConsoleLog;
use crate::editor::{EditOp, do_op};
use crate::font::UiFont;
use crate::i18n::{sink_label, source_label, t};
use crate::state::{ActiveLevel, Editor, GameState, TimetableHovered};

// Editor-edge clamps for the numeric fields. Not balance — just "no nonsense":
// ticks are non-negative, lengths positive, and speed must stay below the
// shortest edge (anti-tunneling, `MAX_SPEED_EXCLUSIVE`).
const TICK_MAX: i64 = 99_999;
const LEN_MIN: i64 = 1;
const LEN_MAX: i64 = 9_999;
const SPEED_MIN: i64 = 1;
const SPEED_MAX: i64 = stellwerk_sim::units::segment_lengths::MAX_SPEED_EXCLUSIVE - 1;
/// A stop must actually dwell, so its duration is clamped ≥ 1 tick.
const DWELL_MIN: i64 = 1;
/// Dwell seeded when a train first gains a freight stop.
const DEFAULT_DWELL: u64 = 30;
/// Row label tint for a freight train (has a platform stop).
const FREIGHT_TINT: Color = Color::srgb(0.45, 0.78, 0.88);

/// Which numeric column of a schedule row a [`NumericField`] edits.
#[derive(Clone, Copy)]
enum SchedFieldKind {
    Depart,
    Due,
    Speed,
    Length,
    Dwell,
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

/// The scrollable region holding the timetable rows (title stays fixed above).
/// Marks the node whose hover gates the wheel and whose `ScrollPosition` moves.
#[derive(Component)]
struct ScheduleScroll;

/// Logical pixels scrolled per wheel line (≈ two rows).
const SCROLL_STEP: f32 = 40.0;

#[derive(Component, Clone, Copy)]
pub(super) enum SchedAction {
    Add,
    Duplicate(usize),
    Remove(usize),
    CycleSource(usize),
    CycleSink(usize),
    CycleClass(usize),
    /// Cycle the freight stop: Kein Halt → B0 → B1 → … → Kein Halt.
    CyclePlatform(usize),
}

pub(super) struct SchedulePanelPlugin;

impl Plugin for SchedulePanelPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<TimetableHovered>()
            .add_systems(
                Update,
                (
                    rebuild_schedule_panel.run_if(resource_exists_and_changed::<ActiveLevel>),
                    schedule_clicks,
                    schedule_field_commits,
                )
                    .run_if(in_state(GameState::Edit)),
            )
            // Ungated so the hover flag resets to false once the panel is gone
            // (leaving Edit), or the board would stay un-zoomable.
            .add_systems(Update, (timetable_hover, timetable_scroll));
    }
}

/// Mirror the scroll region's hover into [`TimetableHovered`] (the wheel/zoom
/// gate), like the console's `console_hover`.
fn timetable_hover(
    region: Query<&Interaction, With<ScheduleScroll>>,
    mut hovered: ResMut<TimetableHovered>,
) {
    let over = region.iter().any(|i| !matches!(i, Interaction::None));
    if hovered.0 != over {
        hovered.0 = over;
    }
}

/// Wheel-scroll the timetable while hovered (mirrors `console_scroll`). Bevy
/// clamps an out-of-range `ScrollPosition` on the next layout pass.
fn timetable_scroll(
    mut wheel: MessageReader<MouseWheel>,
    hovered: Res<TimetableHovered>,
    mut region: Query<&mut ScrollPosition, With<ScheduleScroll>>,
) {
    let delta: f32 = wheel
        .read()
        .map(|e| match e.unit {
            MouseScrollUnit::Line => e.y,
            MouseScrollUnit::Pixel => e.y / 100.0,
        })
        .sum();
    if !hovered.0 || delta == 0.0 {
        return;
    }
    for mut pos in &mut region {
        // Wheel up (delta > 0) scrolls toward the top → smaller y offset.
        pos.0.y = (pos.0.y - delta * SCROLL_STEP).max(0.0);
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
    let sink_name = |level: &stellwerk_sim::Level, id: SinkId| {
        level
            .sinks
            .iter()
            .find(|s| s.id == id)
            .map_or_else(|| format!("Z{}", id.0), |s| sink_label(s.id.0, &s.label))
    };
    let src_label = |level: &stellwerk_sim::Level, id: SourceId| {
        level
            .sources
            .iter()
            .find(|s| s.id == id)
            .map_or_else(|| format!("Q{}", id.0), |s| source_label(s.id.0, &s.label))
    };
    commands.entity(root).with_children(|panel| {
        panel.spawn(text_bundle(&font, t("schedule.title"), 15.0, TEXT_BRIGHT));
        // Rows live in a scroll region: a long timetable wheel-scrolls like the
        // console (see `timetable_scroll`); the title above stays fixed.
        panel
            .spawn((
                Node {
                    flex_direction: FlexDirection::Column,
                    max_height: Val::Vh(42.0),
                    overflow: Overflow::scroll_y(),
                    ..default()
                },
                ScrollPosition::default(),
                Interaction::default(),
                ScheduleScroll,
            ))
            .with_children(|list| {
        if !sandbox {
            for entry in &level.schedule {
                list.spawn((
                    text_bundle(
                        &font,
                        format!(
                            "{}{} · {}{} · {} → {} · {}{} · {}{} · {}{} · {}{}",
                            t("schedule.train"),
                            entry.train.0,
                            t("schedule.class"),
                            entry.class.0,
                            src_label(&level, entry.source),
                            sink_name(&level, entry.sink),
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
            list
                .spawn(Node {
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    // Wrap the row's controls within the capped panel width
                    // instead of overflowing toward the console.
                    flex_wrap: FlexWrap::Wrap,
                    ..default()
                })
                .with_children(|r| {
                    // Freight rows (a platform stop) are tinted so class reads at
                    // a glance in the timetable, matching the board's teal train.
                    let train_tint = if entry.stop.is_some() {
                        FREIGHT_TINT
                    } else {
                        TEXT_DIM
                    };
                    r.spawn(text_bundle(
                        &font,
                        format!("{} {}", t("common.train"), entry.train.0),
                        13.0,
                        train_tint,
                    ));
                    small_button(
                        r,
                        &font,
                        &src_label(&level, entry.source),
                        SchedAction::CycleSource(row),
                    );
                    small_button(
                        r,
                        &font,
                        &format!("→{}", sink_name(&level, entry.sink)),
                        SchedAction::CycleSink(row),
                    );
                    small_button(
                        r,
                        &font,
                        &format!("K{}", entry.class.0),
                        SchedAction::CycleClass(row),
                    );
                    // depart/due/speed/length are now typed, not cycled
                    // Prefix label + focusable numeric field.
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
                    // Freight stop: cycle the target platform; the dwell field
                    // only appears once a stop is set.
                    let stop_label = match entry.stop {
                        None => "Halt —".to_string(),
                        Some(s) => format!("Halt B{}", s.platform.0),
                    };
                    small_button(r, &font, &stop_label, SchedAction::CyclePlatform(row));
                    if let Some(s) = entry.stop {
                        field(r, "dwell", s.dwell.0 as i64, DWELL_MIN, TICK_MAX, SchedFieldKind::Dwell);
                    }
                    small_button(r, &font, "dup", SchedAction::Duplicate(row));
                    small_button(r, &font, "×", SchedAction::Remove(row));
                });
        }
            });
    });
}

/// Cycles a freight stop `Kein Halt → B(ids[0]) → … → Kein Halt`. A fresh stop
/// is seeded with [`DEFAULT_DWELL`]; moving between platforms keeps the current
/// dwell. `None` (no stop) when the level defines no platforms.
fn next_stop(cur: Option<PlatformStop>, ids: &[u32]) -> Option<PlatformStop> {
    let first = *ids.first()?;
    match cur {
        None => Some(PlatformStop {
            platform: PlatformId(first),
            dwell: Tick(DEFAULT_DWELL),
        }),
        Some(s) => match ids.iter().position(|&v| v == s.platform.0) {
            Some(i) if i + 1 < ids.len() => Some(PlatformStop {
                platform: PlatformId(ids[i + 1]),
                dwell: s.dwell,
            }),
            // Past the last platform (or an unknown one) → back to Kein Halt.
            _ => None,
        },
    }
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
    mut log: ResMut<ConsoleLog>,
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
        // Each edit becomes one invertible op on the shared undo stack; the
        // level is mutated only via `do_op` below.
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
                    // Tell the player WHAT is missing — in the console now.
                    log.warn(t(key));
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
                    stop: None,
                };
                Some(EditOp::ScheduleInsert {
                    row: active.level.schedule.len(),
                    entry,
                })
            }
            SchedAction::Duplicate(row) => {
                // Clone the row just below itself with a fresh train id, so a
                // whole timetable is built by tweaking copies, not retyping.
                let next_train = active
                    .level
                    .schedule
                    .iter()
                    .map(|e| e.train.0)
                    .max()
                    .map_or(0, |m| m + 1);
                active.level.schedule.get(row).map(|e| {
                    let mut entry = e.clone();
                    entry.train = TrainId(next_train);
                    EditOp::ScheduleInsert {
                        row: row + 1,
                        entry,
                    }
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
            SchedAction::CyclePlatform(row) => {
                let ids: Vec<u32> = active.level.platforms.iter().map(|p| p.id.0).collect();
                edit_row(&active.level, row, |e| e.stop = next_stop(e.stop, &ids))
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
        // Mutate the level via `bypass_change_detection`: a pure value edit must
        // NOT mark `ActiveLevel` as changed, or `rebuild_schedule_panel` would
        // despawn the very field the player just tabbed into and drop keyboard
        // focus mid-edit. The field already shows the committed value
        // (`numeric_field_render`); undo and live validation still fire off the
        // editor's own change detection (`do_op` mutates `Editor`). Structural
        // edits (add/remove/cycle) keep normal change detection so the panel
        // does rebuild for them.
        let level = &mut active.bypass_change_detection().level;
        let op = edit_row(level, row, |e| match kind {
            SchedFieldKind::Depart => e.depart = Tick(value as u64),
            SchedFieldKind::Due => e.due = Tick(value as u64),
            SchedFieldKind::Speed => e.speed = Speed(value),
            SchedFieldKind::Length => e.length = Len(value),
            SchedFieldKind::Dwell => {
                if let Some(s) = e.stop.as_mut() {
                    s.dwell = Tick(value.max(DWELL_MIN) as u64);
                }
            }
        });
        if let Some(op) = op {
            do_op(&mut editor, level, op);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stop_cycles_none_through_platforms_back_to_none() {
        // No platforms → never a stop.
        assert_eq!(next_stop(None, &[]), None);

        // None → first platform, seeded with the default dwell.
        let s0 = next_stop(None, &[10, 20]).expect("first platform");
        assert_eq!(s0.platform, PlatformId(10));
        assert_eq!(s0.dwell, Tick(DEFAULT_DWELL));

        // Advance to the next platform, keeping the (edited) dwell.
        let edited = PlatformStop {
            platform: PlatformId(10),
            dwell: Tick(55),
        };
        let s1 = next_stop(Some(edited), &[10, 20]).expect("second platform");
        assert_eq!(s1.platform, PlatformId(20));
        assert_eq!(s1.dwell, Tick(55), "dwell carries across platforms");

        // Past the last platform → back to Kein Halt.
        assert_eq!(next_stop(Some(s1), &[10, 20]), None);

        // An unknown platform id (level edited underneath) → Kein Halt, no panic.
        let stale = PlatformStop {
            platform: PlatformId(99),
            dwell: Tick(5),
        };
        assert_eq!(next_stop(Some(stale), &[10, 20]), None);
    }
}
