//! Edit HUD: level name, tool/hint lines, diagnostics, solution slots, the
//! start button (Space/Enter) and the panel root nodes.

use bevy::prelude::*;
use stellwerk_codes::Payload;
use stellwerk_sim::ValidationError;

use super::schedule_panel::{SchedAction, SchedulePanelRoot, rebuild_schedule_panel};
use super::switch_panel::{SwitchPanelRoot, rebuild_switch_panel};
use super::widgets::{
    BUTTON_BG_BLOCKED, BUTTON_BG_PRIMARY, ButtonBase, PANEL_BG, TEXT_BRIGHT, TEXT_DIM, button,
    despawn_all, set_text, small_button, text_bundle,
};
use crate::font::UiFont;
use crate::i18n::{level_name, t};
use crate::levels::{Progress, SOLUTION_SLOTS, save_sandbox};
use crate::state::{ActiveLevel, Diagnostics, Editor, GameState, Tool};

/// Marker for everything despawned when leaving Edit (incl. panel roots).
#[derive(Component)]
pub(super) struct UiEdit;

#[derive(Component)]
struct StartButton;
#[derive(Component)]
struct DiagText;
#[derive(Component)]
struct ToolText;
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
        app.add_systems(
            OnEnter(GameState::Edit),
            (
                spawn_edit_hud,
                rebuild_switch_panel,
                rebuild_schedule_panel,
                fill_edit_texts,
            )
                .chain(),
        )
        .add_systems(OnExit(GameState::Edit), despawn_all::<UiEdit>)
        .add_systems(
            Update,
            (
                update_edit_texts,
                start_button,
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
            // Operating order (GDD §8.1): the puzzle statement, shown right on
            // the desk. Campaign levels carry one; the sandbox does not.
            if !brief.is_empty() {
                c.spawn(text_bundle(&font, brief, 14.0, TEXT_BRIGHT));
            }
            c.spawn((text_bundle(&font, String::new(), 14.0, TEXT_DIM), ToolText));
            c.spawn(text_bundle(&font, t("edit.hints"), 13.0, TEXT_DIM));
            c.spawn((
                text_bundle(&font, String::new(), 14.0, Color::srgb(1.0, 0.45, 0.35)),
                DiagText,
            ));
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
    // Always present: campaign levels show the timetable read-only — who
    // must go where is part of the puzzle statement, not something to
    // discover from a failed run. The sandbox panel is editable.
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(10.0),
            bottom: Val::Px(10.0),
            flex_direction: FlexDirection::Column,
            padding: UiRect::all(Val::Px(8.0)),
            ..default()
        },
        BackgroundColor(PANEL_BG),
        Interaction::default(),
        SchedulePanelRoot,
        UiEdit,
    ));
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

/// Diagnostics line (first errors, then reachability warnings).
fn diag_line(diagnostics: &Diagnostics) -> String {
    let mut lines = Vec::new();
    for error in diagnostics.errors.iter().take(3) {
        lines.push(format!("✗ {}", valerr_text(error)));
    }
    if diagnostics.errors.len() > 3 {
        lines.push(format!(
            "… +{} {}",
            diagnostics.errors.len() - 3,
            t("edit.more_errors")
        ));
    }
    for unreachable in diagnostics.unreachable.iter().take(2) {
        lines.push(format!("{}{}", t("edit.unreachable"), unreachable.train.0));
    }
    lines.join("\n")
}

/// One-shot fill on entering Edit: the HUD text entities are respawned on every
/// entry (incl. Result → "back to edit", which changes no resource), so the
/// guarded per-frame updater alone would leave them empty.
fn fill_edit_texts(
    editor: Res<Editor>,
    diagnostics: Res<Diagnostics>,
    active: Option<Res<ActiveLevel>>,
    mut tool_texts: Query<&mut Text, (With<ToolText>, Without<DiagText>)>,
    mut diag_texts: Query<&mut Text, (With<DiagText>, Without<ToolText>)>,
) {
    let sandbox = active.as_ref().is_some_and(|a| a.sandbox);
    if let Ok(mut text) = tool_texts.single_mut() {
        set_text(&mut text, tool_line(editor.tool, sandbox));
    }
    if let Ok(mut text) = diag_texts.single_mut() {
        set_text(&mut text, diag_line(&diagnostics));
    }
}

/// Per-frame refresh, but each block only rebuilds its string when its inputs
/// actually changed — idle frames allocate nothing. The tool is switched via
/// `bypass_change_detection` (so it doesn't trigger the board rebuild), hence a
/// `Local` compare instead of `editor.is_changed()`; diagnostics use normal
/// change detection.
fn update_edit_texts(
    editor: Res<Editor>,
    diagnostics: Res<Diagnostics>,
    active: Option<Res<ActiveLevel>>,
    mut tool_texts: Query<&mut Text, (With<ToolText>, Without<DiagText>)>,
    mut diag_texts: Query<&mut Text, (With<DiagText>, Without<ToolText>)>,
    mut last_tool: Local<Option<(Tool, bool)>>,
) {
    let sandbox = active.as_ref().is_some_and(|a| a.sandbox);
    if *last_tool != Some((editor.tool, sandbox)) {
        *last_tool = Some((editor.tool, sandbox));
        if let Ok(mut text) = tool_texts.single_mut() {
            set_text(&mut text, tool_line(editor.tool, sandbox));
        }
    }
    if diagnostics.is_changed()
        && let Ok(mut text) = diag_texts.single_mut()
    {
        set_text(&mut text, diag_line(&diagnostics));
    }
}

fn start_button(
    mut interactions: Query<(&Interaction, &mut ButtonBase, &Children), With<StartButton>>,
    mut texts: Query<&mut Text>,
    keys: Res<ButtonInput<KeyCode>>,
    diagnostics: Res<Diagnostics>,
    mut next: ResMut<NextState<GameState>>,
) {
    let allowed = diagnostics.start_allowed();
    let mut clicked = keys.just_pressed(KeyCode::Enter) || keys.just_pressed(KeyCode::Space);
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
            clicked = true;
        }
    }
    if clicked && allowed {
        next.set(GameState::Run);
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
) {
    let Some(active) = active else { return };
    if interactions.iter().any(|i| *i == Interaction::Pressed) {
        let code = stellwerk_codes::encode(&Payload::Level {
            level: active.level.clone(),
        });
        if let Err(e) = std::fs::write("stellwerk_code.txt", code) {
            warn!("export failed: {e}");
        } else {
            info!("level code written to stellwerk_code.txt");
        }
        save_sandbox(&active.level);
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
    }
}

/// Every key [`valerr_text`] can emit — kept beside the match so the i18n
/// coverage checker (see `crate::i18n` tests) can assert all of them resolve
/// in both languages. MUST stay in sync with the arms below; adding a
/// [`ValidationError`] variant breaks the exhaustive match and reminds you.
#[cfg(test)]
pub(crate) const VALERR_KEYS: &[&str] = &[
    "valerr.illegal_pair",
    "valerr.duplicate_piece",
    "valerr.switch_clash",
    "valerr.switch_angle",
    "valerr.switch_default",
    "valerr.switch_rule_branch",
    "valerr.switch_rule_sink",
    "valerr.switch_not_exclusive",
    "valerr.duplicate_switch",
    "valerr.signal_off_track",
    "valerr.duplicate_signal",
    "valerr.junction_no_switch",
    "valerr.connector_reused",
    "valerr.outside_buildable",
    "valerr.dup_source_id",
    "valerr.dup_sink_id",
    "valerr.source_off_track",
    "valerr.sink_off_track",
    "valerr.dup_train_id",
    "valerr.unknown_source",
    "valerr.unknown_sink",
    "valerr.non_positive_length",
    "valerr.non_positive_speed",
    "valerr.speed_too_high",
    "valerr.due_before_depart",
];

/// Localized validation-error line. The sim crate's `Display` stays English
/// (logs/tests); the player-facing text is translated here, with the
/// concrete cell/id appended in Rust (the i18n shim has no placeholders).
fn valerr_text(error: &ValidationError) -> String {
    use ValidationError::*;
    let at = |cell: &stellwerk_sim::grid::Cell| format!("({}, {})", cell.x, cell.y);
    match error {
        IllegalPiecePair { cell, .. } => format!("{} {}", t("valerr.illegal_pair"), at(cell)),
        DuplicatePiece { cell, .. } => format!("{} {}", t("valerr.duplicate_piece"), at(cell)),
        SwitchConnectorClash { cell } => format!("{} {}", t("valerr.switch_clash"), at(cell)),
        SwitchBranchAngle { cell, .. } => format!("{} {}", t("valerr.switch_angle"), at(cell)),
        SwitchDefaultOutOfRange { cell } => format!("{} {}", t("valerr.switch_default"), at(cell)),
        SwitchRuleBranchOutOfRange { cell } => {
            format!("{} {}", t("valerr.switch_rule_branch"), at(cell))
        }
        SwitchRuleUnknownSink { cell, .. } => {
            format!("{} {}", t("valerr.switch_rule_sink"), at(cell))
        }
        SwitchCellNotExclusive { cell } => {
            format!("{} {}", t("valerr.switch_not_exclusive"), at(cell))
        }
        DuplicateSwitch { cell } => format!("{} {}", t("valerr.duplicate_switch"), at(cell)),
        SignalOffTrack { cell, .. } => format!("{} {}", t("valerr.signal_off_track"), at(cell)),
        DuplicateSignal { cell, .. } => format!("{} {}", t("valerr.duplicate_signal"), at(cell)),
        JunctionWithoutSwitch { point } => {
            format!("{} ({}, {})", t("valerr.junction_no_switch"), point.x, point.y)
        }
        ConnectorReused { cell, .. } => format!("{} {}", t("valerr.connector_reused"), at(cell)),
        OutsideBuildable { cell } => format!("{} {}", t("valerr.outside_buildable"), at(cell)),
        DuplicateSourceId { id } => format!("{} {}", t("valerr.dup_source_id"), id.0),
        DuplicateSinkId { id } => format!("{} {}", t("valerr.dup_sink_id"), id.0),
        SourceOffTrack { id } => format!("{} {}", t("valerr.source_off_track"), id.0),
        SinkOffTrack { id } => format!("{} {}", t("valerr.sink_off_track"), id.0),
        DuplicateTrainId { train } => format!("{} {}", t("valerr.dup_train_id"), train.0),
        UnknownSource { train, source } => {
            format!("{} {} ({})", t("valerr.unknown_source"), train.0, source.0)
        }
        UnknownSink { train, sink } => {
            format!("{} {} ({})", t("valerr.unknown_sink"), train.0, sink.0)
        }
        NonPositiveLength { train } => format!("{} {}", t("valerr.non_positive_length"), train.0),
        NonPositiveSpeed { train } => format!("{} {}", t("valerr.non_positive_speed"), train.0),
        SpeedTooHigh { train } => format!("{} {}", t("valerr.speed_too_high"), train.0),
        DueBeforeDepart { train } => format!("{} {}", t("valerr.due_before_depart"), train.0),
    }
}
