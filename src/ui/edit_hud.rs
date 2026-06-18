//! Edit HUD: level name, tool/hint lines, diagnostics, solution slots, the
//! start button (Space/Enter) and the panel root nodes.

use std::collections::HashSet;

use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use stellwerk_codes::Payload;
use stellwerk_sim::grid::Cell;

use crate::clipboard::CopyOutcome;
use crate::console::{ConsoleLog, Severity};
use super::schedule_panel::{SchedAction, SchedulePanelRoot, rebuild_schedule_panel};
use super::station_panel::{StationPanelRoot, rebuild_station_panel};
use super::valerr::{build_issue_text, unreachable_text, valerr_text};
use super::switch_panel::{SwitchPanelRoot, rebuild_switch_panel};
use super::widgets::{
    BUTTON_BG_BLOCKED, BUTTON_BG_PRIMARY, ButtonBase, PANEL_BG, TEXT_BRIGHT, TEXT_DIM, button,
    despawn_all, set_text, small_button, text_bundle,
};
use crate::font::UiFont;
use crate::i18n::{level_name, t};
use crate::levels::{Progress, SANDBOX_ID, SOLUTION_SLOTS, save_sandbox};
use crate::state::{
    ActiveLevel, Diagnostics, Editor, FocusedField, GameState, Tool, no_field_focused, not_paused,
};

/// Marker for everything despawned when leaving Edit (incl. panel roots).
#[derive(Component)]
pub(super) struct UiEdit;

#[derive(Component)]
struct StartButton;

/// Diagnostic lines as of the last console mirror, by text. The mirror diffs the
/// current diagnostics against this to log appearances and resolutions as
/// discrete events. Cleared on entering Edit (see `fill_edit_texts`).
#[derive(Resource, Default)]
struct LastDiag(HashSet<String>);

#[derive(Component)]
struct ToolText;
/// Live cell coordinate under the cursor — a building aid for hand-editing the
/// `.ron` (fixed track, source/sink cells) without eyeballing the grid.
#[derive(Component)]
struct CoordsText;
#[derive(Component)]
struct ExportLevelButton;

#[derive(Component, Clone, Copy)]
enum SlotAction {
    Save(usize),
    Load(usize),
}

pub(super) struct EditHudPlugin;

impl Plugin for EditHudPlugin {
    fn build(&self, app: &mut App) {
        // Chained so the panel roots spawned by the HUD exist (deferred
        // commands flush between ordered systems) before the initial
        // fill — otherwise the panels stay empty when re-entering Edit
        // without an Editor/ActiveLevel change (e.g. back from Result).
        app.init_resource::<LastDiag>().add_systems(
            OnEnter(GameState::Edit),
            (
                spawn_edit_hud,
                rebuild_switch_panel,
                rebuild_schedule_panel,
                rebuild_station_panel,
                fill_edit_texts,
            )
                .chain(),
        )
        .add_systems(OnExit(GameState::Edit), despawn_all::<UiEdit>)
        .add_systems(
            Update,
            (
                update_edit_texts,
                update_coords,
                mirror_diagnostics_to_console.run_if(resource_changed::<Diagnostics>),
                // Gated so Space/Enter cannot start the run behind the pause
                // menu (the overlay already absorbs the start button click), nor
                // while typing into a numeric field (Enter commits the field).
                start_button.run_if(not_paused).run_if(no_field_focused),
                slot_clicks,
                export_level_click,
            )
                .run_if(in_state(GameState::Edit)),
        );
    }
}

fn spawn_edit_hud(
    mut commands: Commands,
    ui_font: Res<UiFont>,
    active: Option<Res<ActiveLevel>>,
) {
    let font = ui_font.0.clone();
    let (name, brief, sandbox) = active
        .map(|a| {
            (
                level_name(&a.id, &a.level.name),
                crate::i18n::briefing(&a.id, &a.briefing),
                a.sandbox,
            )
        })
        .unwrap_or_default();
    // The container nodes carry `Interaction` so the board pointer can tell
    // "this click landed on UI" — also for clicks BETWEEN the buttons.
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(10.0),
                top: Val::Px(8.0),
                flex_direction: FlexDirection::Column,
                ..default()
            },
            Interaction::default(),
            UiEdit,
        ))
        .with_children(|c| {
            c.spawn(text_bundle(&font, name, 22.0, TEXT_BRIGHT));
            // Operating order: the puzzle statement, shown right on
            // the desk. Campaign levels carry one; the sandbox does not.
            if !brief.is_empty() {
                c.spawn(text_bundle(&font, brief, 14.0, TEXT_BRIGHT));
            }
            c.spawn((text_bundle(&font, String::new(), 14.0, TEXT_DIM), ToolText));
            c.spawn(text_bundle(&font, t("edit.hints"), 13.0, TEXT_DIM));
            c.spawn((text_bundle(&font, String::new(), 13.0, TEXT_DIM), CoordsText));
            // Diagnostics no longer live here — they go to the console
            // (`mirror_diagnostics_to_console`), clickable to recentre the board.
            // Solution slots.
            c.spawn(Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                ..default()
            })
            .with_children(|row| {
                row.spawn(text_bundle(&font, t("edit.slots"), 13.0, TEXT_DIM));
                for i in 0..SOLUTION_SLOTS {
                    small_button(
                        row,
                        &font,
                        &format!("{}{}", t("edit.save_slot"), i + 1),
                        SlotAction::Save(i),
                    );
                }
                for i in 0..SOLUTION_SLOTS {
                    small_button(
                        row,
                        &font,
                        &format!("{}{}", t("edit.load_slot"), i + 1),
                        SlotAction::Load(i),
                    );
                }
            });
            if sandbox {
                c.spawn(Node {
                    flex_direction: FlexDirection::Row,
                    ..default()
                })
                .with_children(|row| {
                    small_button(row, &font, &t("edit.export_level"), ExportLevelButton);
                    small_button(row, &font, &t("edit.add_train"), SchedAction::Add);
                });
            }
        });
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                right: Val::Px(10.0),
                top: Val::Px(8.0),
                ..default()
            },
            Interaction::default(),
            UiEdit,
        ))
        .with_children(|c| {
            button(c, &font, &t("edit.start"), BUTTON_BG_PRIMARY, StartButton);
        });
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            right: Val::Px(10.0),
            top: Val::Px(64.0),
            flex_direction: FlexDirection::Column,
            padding: UiRect::all(Val::Px(8.0)),
            ..default()
        },
        BackgroundColor(Color::NONE),
        Interaction::default(),
        SwitchPanelRoot,
        UiEdit,
    ));
    // Bottom-left group: the timetable and (sandbox) the station-rename panel sit
    // side by side in one flex Row so they can never overlap — and so the
    // station panel no longer collides with the dev save panel bottom-right.
    // Only the wrapper carries `UiEdit` (despawn is recursive); each child keeps
    // its own `Interaction` so board clicks are absorbed (`over_ui` reads every
    // `Interaction` flat, regardless of hierarchy).
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(10.0),
                bottom: Val::Px(10.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::FlexEnd,
                column_gap: Val::Px(10.0),
                ..default()
            },
            UiEdit,
        ))
        .with_children(|group| {
            // Always present: campaign levels show the timetable read-only — who
            // must go where is part of the puzzle statement, not something to
            // discover from a failed run. The sandbox panel is editable.
            group.spawn((
                Node {
                    flex_direction: FlexDirection::Column,
                    padding: UiRect::all(Val::Px(8.0)),
                    ..default()
                },
                BackgroundColor(PANEL_BG),
                Interaction::default(),
                SchedulePanelRoot,
            ));
            // Sandbox-only content; collapses to `Display::None` in campaign so it
            // leaves no phantom padded box beside the timetable (see
            // `rebuild_station_panel`).
            group.spawn((
                Node {
                    flex_direction: FlexDirection::Column,
                    padding: UiRect::all(Val::Px(8.0)),
                    ..default()
                },
                BackgroundColor(PANEL_BG),
                Interaction::default(),
                StationPanelRoot,
            ));
        });
}

/// Tool/sandbox status line.
fn tool_line(tool: Tool, sandbox: bool) -> String {
    let extra = if sandbox {
        t("edit.tools_sandbox")
    } else {
        String::new()
    };
    format!(
        "{}{extra}   |   {}{}",
        t("edit.tools"),
        t("edit.tool_label"),
        t(tool_key(tool))
    )
}

/// World position to recentre on for a located error, or `None` for the
/// schedule/id errors that have no single board cell. The source/sink-off-track
/// errors carry only an id, so they need the `level` to resolve their cell.
/// Exhaustive on purpose — a new [`stellwerk_sim::ValidationError`] variant must
/// be classified here.
fn error_world(
    error: &stellwerk_sim::ValidationError,
    level: Option<&stellwerk_sim::Level>,
) -> Option<Vec2> {
    use stellwerk_sim::ValidationError::*;
    let cell = match error {
        IllegalPiecePair { cell, .. }
        | DuplicatePiece { cell, .. }
        | SwitchConnectorClash { cell }
        | SwitchBranchAngle { cell, .. }
        | SwitchDefaultOutOfRange { cell }
        | SwitchRuleBranchOutOfRange { cell }
        | SwitchRuleUnknownSink { cell, .. }
        | SwitchCellNotExclusive { cell }
        | DuplicateSwitch { cell }
        | SignalOffTrack { cell, .. }
        | DuplicateSignal { cell, .. }
        | ConnectorReused { cell, .. }
        | OutsideBuildable { cell } => *cell,
        JunctionWithoutSwitch { point } => return Some(crate::board::point_world(*point)),
        SourceOffTrack { id } => level?.sources.iter().find(|s| s.id == *id)?.cell,
        SinkOffTrack { id } => level?.sinks.iter().find(|s| s.id == *id)?.cell,
        DuplicateSourceId { .. }
        | DuplicateSinkId { .. }
        | DuplicateTrainId { .. }
        | UnknownSource { .. }
        | UnknownSink { .. }
        | NonPositiveLength { .. }
        | NonPositiveSpeed { .. }
        | SpeedTooHigh { .. }
        | DueBeforeDepart { .. } => return None,
    };
    Some(crate::board::cell_world(cell))
}

/// World position for a reachability warning: the cell of the train's source
/// (resolved through the schedule), so the console line can recentre there.
fn unreachable_world(
    unreachable: &stellwerk_sim::Unreachable,
    level: Option<&stellwerk_sim::Level>,
) -> Option<Vec2> {
    let level = level?;
    let entry = level
        .schedule
        .iter()
        .find(|e| e.train == unreachable.train)?;
    let source = level.sources.iter().find(|s| s.id == entry.source)?;
    Some(crate::board::cell_world(source.cell))
}

/// Mirrors diagnostics into the in-level console as an event journal: a problem
/// is logged when it appears (errors/build-blocks as errors, reachability as
/// warnings, all with a camera jump where one is resolvable) and an info
/// "resolved" line when it clears. The set is diffed against [`LastDiag`], so a
/// genuinely re-appearing problem logs again — unlike a monotonic de-dup. Logging
/// is skipped while a track drag is in progress (it flaps the diagnostics every
/// cell); the settled state is logged once the drag finishes.
fn mirror_diagnostics_to_console(
    diagnostics: Res<Diagnostics>,
    editor: Res<Editor>,
    active: Option<Res<ActiveLevel>>,
    mut log: ResMut<ConsoleLog>,
    mut last: ResMut<LastDiag>,
) {
    if editor.drag.is_some() {
        return;
    }
    let level = active.as_ref().map(|a| &a.level);
    // Current diagnostics as (text, severity, jump), in display order.
    let mut current: Vec<(String, Severity, Option<Vec2>)> = Vec::new();
    for issue in &diagnostics.build_issues {
        current.push((build_issue_text(issue), Severity::Error, None));
    }
    for error in &diagnostics.errors {
        current.push((valerr_text(error), Severity::Error, error_world(error, level)));
    }
    for unreachable in &diagnostics.unreachable {
        current.push((
            unreachable_text(unreachable),
            Severity::Warn,
            unreachable_world(unreachable, level),
        ));
    }
    let current_texts: HashSet<String> = current.iter().map(|(text, ..)| text.clone()).collect();
    // Newly appeared since last diff → log with its severity and jump target.
    for (text, severity, jump) in &current {
        if !last.0.contains(text) {
            log.push_at(*severity, text.clone(), *jump);
        }
    }
    // Resolved since last diff → a self-correcting "behoben" info line.
    for text in last.0.difference(&current_texts) {
        log.info(format!("{} {text}", t("console.resolved")));
    }
    last.0 = current_texts;
}

/// One-shot fill on entering Edit: the HUD text entities are respawned on every
/// entry (incl. Result → "back to edit", which changes no resource), so the
/// guarded per-frame updater alone would leave them empty.
fn fill_edit_texts(
    editor: Res<Editor>,
    active: Option<Res<ActiveLevel>>,
    mut focus: ResMut<FocusedField>,
    mut last: ResMut<LastDiag>,
    mut tool_texts: Query<&mut Text, With<ToolText>>,
) {
    // Drop any stale focus from a previous Edit session (the fields are respawned
    // on entry). The console mirror's diff set resets too, so a different level
    // starts logging fresh.
    focus.0 = None;
    last.0.clear();
    let sandbox = active.as_ref().is_some_and(|a| a.sandbox);
    if let Ok(mut text) = tool_texts.single_mut() {
        set_text(&mut text, tool_line(editor.tool, sandbox));
    }
}

/// Per-frame refresh of the tool line, rebuilt only when the tool/sandbox flag
/// changes — idle frames allocate nothing. The tool is switched via
/// `bypass_change_detection` (so it doesn't trigger the board rebuild), hence a
/// `Local` compare instead of `editor.is_changed()`. Diagnostics are handled by
/// `rebuild_diag_panel` (gated on `Diagnostics` change).
fn update_edit_texts(
    editor: Res<Editor>,
    active: Option<Res<ActiveLevel>>,
    mut tool_texts: Query<&mut Text, With<ToolText>>,
    mut last_tool: Local<Option<(Tool, bool)>>,
) {
    let sandbox = active.as_ref().is_some_and(|a| a.sandbox);
    if *last_tool != Some((editor.tool, sandbox)) {
        *last_tool = Some((editor.tool, sandbox));
        if let Ok(mut text) = tool_texts.single_mut() {
            set_text(&mut text, tool_line(editor.tool, sandbox));
        }
    }
}

/// Shows the cell under the cursor, refreshed only when it changes (no
/// per-frame allocation while the mouse is still). Blank when off the board.
fn update_coords(
    windows: Query<&Window, With<PrimaryWindow>>,
    cameras: Query<(&Camera, &GlobalTransform), With<crate::camera::MainCamera>>,
    mut texts: Query<&mut Text, With<CoordsText>>,
    mut last: Local<Option<Cell>>,
) {
    let cell = crate::camera::cursor_world(&windows, &cameras).map(crate::board::world_cell);
    if *last == cell {
        return;
    }
    *last = cell;
    if let Ok(mut text) = texts.single_mut() {
        set_text(
            &mut text,
            cell.map_or(String::new(), |c| format!("({}, {})", c.x, c.y)),
        );
    }
}

fn start_button(
    mut interactions: Query<(&Interaction, &mut ButtonBase, &Children), With<StartButton>>,
    mut texts: Query<&mut Text>,
    keys: Res<ButtonInput<KeyCode>>,
    diagnostics: Res<Diagnostics>,
    mut next: ResMut<NextState<GameState>>,
    mut log: ResMut<ConsoleLog>,
    mut mouse_was_pressed: Local<bool>,
) {
    let allowed = diagnostics.start_allowed();
    let mut mouse_pressed = false;
    for (interaction, mut base, children) in &mut interactions {
        let target = if allowed {
            BUTTON_BG_PRIMARY
        } else {
            BUTTON_BG_BLOCKED
        };
        // Write through ButtonBase (not BackgroundColor directly) so the
        // hover/press feedback keeps working — and only on actual change.
        if base.0 != target {
            base.0 = target;
        }
        if let Some(&child) = children.first()
            && let Ok(mut text) = texts.get_mut(child)
        {
            set_text(
                &mut text,
                if allowed {
                    t("edit.start")
                } else {
                    t("edit.start_blocked")
                },
            );
        }
        if *interaction == Interaction::Pressed {
            mouse_pressed = true;
        }
    }
    // Edge-detect the mouse press so a held button logs once, not per frame
    // (the keyboard keys are already edge-triggered).
    let mouse_edge = mouse_pressed && !*mouse_was_pressed;
    *mouse_was_pressed = mouse_pressed;
    // Space + the button only — NOT Enter. Enter commits a focused field and
    // clears its focus in the same frame, so an Enter that ran before this
    // system would also start the run (no_field_focused would already be true).
    // Space is safe: it never clears field focus, so a focused field gates it.
    let clicked = keys.just_pressed(KeyCode::Space) || mouse_edge;
    if !clicked {
        return;
    }
    if allowed {
        next.set(GameState::Run);
    } else if let Some(msg) = first_blocking_message(&diagnostics) {
        log.error(msg);
    }
}

/// The first build block / validation error, localized — the line the console
/// shows when the player hits START on a level that cannot run yet.
fn first_blocking_message(diagnostics: &Diagnostics) -> Option<String> {
    if let Some(issue) = diagnostics.build_issues.first() {
        Some(build_issue_text(issue))
    } else {
        diagnostics.errors.first().map(valerr_text)
    }
}

fn slot_clicks(
    mut interactions: Query<(&Interaction, &SlotAction), Changed<Interaction>>,
    active: Option<Res<ActiveLevel>>,
    mut editor: ResMut<Editor>,
    mut progress: ResMut<Progress>,
) {
    let Some(active) = active else { return };
    for (interaction, action) in &mut interactions {
        if *interaction != Interaction::Pressed {
            continue;
        }
        match action {
            SlotAction::Save(i) => {
                let layout = editor.layout.clone();
                progress.entry(&active.id).set_slot(*i, layout);
                progress.save();
            }
            SlotAction::Load(i) => {
                if let Some(layout) = progress
                    .levels
                    .get(&active.id)
                    .and_then(|p| p.slot(*i))
                    .cloned()
                {
                    editor.layout = layout;
                    editor.undo.clear();
                    editor.redo.clear();
                    editor.selected_switch = None;
                }
            }
        }
    }
}

fn export_level_click(
    interactions: Query<&Interaction, (Changed<Interaction>, With<ExportLevelButton>)>,
    active: Option<Res<ActiveLevel>>,
    mut log: ResMut<ConsoleLog>,
) {
    let Some(active) = active else { return };
    if interactions.iter().any(|i| *i == Interaction::Pressed) {
        let code = stellwerk_codes::encode(&Payload::Level {
            level: active.level.clone(),
        });
        // English `info!`/`warn!` stay for the dev log; the console line is the
        // localized, player-facing echo.
        match crate::clipboard::copy(&code) {
            CopyOutcome::Clipboard => {
                info!("level code copied to clipboard");
                log.info(t("console.export_ok"));
            }
            CopyOutcome::File(path) => {
                info!("level code written to {}", path.display());
                log.info(t("console.export_ok"));
            }
            CopyOutcome::Failed(e) => {
                warn!("level export failed: {e}");
                log.warn(t("console.export_failed"));
            }
        }
        // Only persist to the sandbox file when this IS the real sandbox. A dev
        // tweaking a campaign level via "open in sandbox" carries the campaign
        // id, and must not clobber the player's actual sandbox save.
        if active.id == SANDBOX_ID {
            save_sandbox(&active.level);
        }
    }
}

/// i18n key for a tool's display name. Exhaustive — a new [`Tool`] variant
/// will not compile until it gets a key here.
pub(crate) fn tool_key(tool: Tool) -> &'static str {
    match tool {
        Tool::Select => "tool.select",
        Tool::Track => "tool.track",
        Tool::Switch => "tool.switch",
        Tool::SignalBlock => "tool.signal_block",
        Tool::SignalChain => "tool.signal_chain",
        Tool::Erase => "tool.erase",
        Tool::Source => "tool.source",
        Tool::Sink => "tool.sink",
        Tool::Block => "tool.block",
    }
}
