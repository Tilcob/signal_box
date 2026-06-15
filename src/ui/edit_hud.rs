//! Edit HUD: level name, tool/hint lines, diagnostics, solution slots, the
//! start button (Space/Enter) and the panel root nodes.

use bevy::prelude::*;
use stellwerk_codes::Payload;

use crate::clipboard::CopyOutcome;
use super::schedule_panel::{SchedAction, SchedulePanelRoot, rebuild_schedule_panel};
use super::valerr::valerr_text;
use super::switch_panel::{SwitchPanelRoot, rebuild_switch_panel};
use super::widgets::{
    BUTTON_BG_BLOCKED, BUTTON_BG_PRIMARY, ButtonBase, PANEL_BG, TEXT_BRIGHT, TEXT_DIM, button,
    despawn_all, set_text, small_button, text_bundle,
};
use crate::font::UiFont;
use crate::i18n::{level_name, t};
use crate::levels::{Progress, SOLUTION_SLOTS, save_sandbox};
use crate::state::{
    ActiveLevel, Diagnostics, EditNotice, Editor, FocusedField, GameState, Tool, no_field_focused,
    not_paused,
};

/// Marker for everything despawned when leaving Edit (incl. panel roots).
#[derive(Component)]
pub(super) struct UiEdit;

#[derive(Component)]
struct StartButton;
#[derive(Component)]
struct DiagText;
/// Transient action-feedback line (e.g. "set a source first"); driven by
/// [`EditNotice`], separate from the validation [`DiagText`].
#[derive(Component)]
struct NoticeText;
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
                tick_edit_notice,
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
            // Amber, below the red validation line: transient "why that action
            // was refused" feedback (M2 restfeature 02 follow-up).
            c.spawn((
                text_bundle(&font, String::new(), 13.0, Color::srgb(1.0, 0.78, 0.35)),
                NoticeText,
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
    mut notice: ResMut<EditNotice>,
    mut focus: ResMut<FocusedField>,
    mut tool_texts: Query<&mut Text, (With<ToolText>, Without<DiagText>)>,
    mut diag_texts: Query<&mut Text, (With<DiagText>, Without<ToolText>)>,
) {
    // Drop any stale notice / focus from a previous Edit session (the text
    // entities and fields are respawned on entry).
    notice.0 = None;
    focus.0 = None;
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

/// Ticks the transient [`EditNotice`] and mirrors it into [`NoticeText`],
/// clearing both when the timer elapses.
fn tick_edit_notice(
    time: Res<Time>,
    mut notice: ResMut<EditNotice>,
    mut texts: Query<&mut Text, With<NoticeText>>,
) {
    let Some((msg, timer)) = notice.0.as_mut() else {
        return;
    };
    timer.tick(time.delta());
    let finished = timer.just_finished();
    let line = if finished { String::new() } else { msg.clone() };
    if let Ok(mut text) = texts.single_mut() {
        set_text(&mut text, line);
    }
    if finished {
        notice.0 = None;
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
        match crate::clipboard::copy(&code) {
            CopyOutcome::Clipboard => info!("level code copied to clipboard"),
            CopyOutcome::File(path) => info!("level code written to {}", path.display()),
            CopyOutcome::Failed(e) => warn!("level export failed: {e}"),
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
