//! The action buttons of the edit info panel: the solution-slot save/load row
//! (every level) and the sandbox level-code export button. The spawn helpers
//! are called by the HUD shell (`super::spawn_edit_hud`) to build the buttons
//! as children of the top-left panel; the click systems live here.

use bevy::prelude::*;
use bevy::text::Font;
use stellwerk_codes::Payload;

use crate::clipboard::CopyOutcome;
use crate::console::ConsoleLog;
use crate::i18n::t;
use crate::levels::{Progress, SANDBOX_ID, SOLUTION_SLOTS, save_sandbox};
use crate::state::{ActiveLevel, Editor, GameState};
use crate::ui::widgets::{TEXT_DIM, small_button, text_bundle};

#[derive(Component)]
struct ExportLevelButton;

#[derive(Component, Clone, Copy)]
enum SlotAction {
    Save(usize),
    Load(usize),
}

pub(super) struct ActionsPlugin;

impl Plugin for ActionsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (slot_clicks, export_level_click).run_if(in_state(GameState::Edit)),
        );
    }
}

/// The solution-slot save/load button row (label + Save N + Load N).
pub(super) fn slots_row(parent: &mut ChildSpawnerCommands, font: &Handle<Font>) {
    parent
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            ..default()
        })
        .with_children(|row| {
            row.spawn(text_bundle(font, t("edit.slots"), 13.0, TEXT_DIM));
            for i in 0..SOLUTION_SLOTS {
                small_button(
                    row,
                    font,
                    &format!("{}{}", t("edit.save_slot"), i + 1),
                    SlotAction::Save(i),
                );
            }
            for i in 0..SOLUTION_SLOTS {
                small_button(
                    row,
                    font,
                    &format!("{}{}", t("edit.load_slot"), i + 1),
                    SlotAction::Load(i),
                );
            }
        });
}

/// The sandbox-only "export level as code" button (caller places it in a row).
pub(super) fn export_button(parent: &mut ChildSpawnerCommands, font: &Handle<Font>) {
    small_button(parent, font, &t("edit.export_level"), ExportLevelButton);
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
    active: Option<ResMut<ActiveLevel>>,
    editor: Res<Editor>,
    mut log: ResMut<ConsoleLog>,
) {
    let Some(mut active) = active else { return };
    if interactions.iter().any(|i| *i == Interaction::Pressed) {
        // Calibrate the timetable to the solution currently on the board, so 0
        // lateness is achievable for it (sandbox authoring): each train's `due`
        // becomes its measured arrival + slack. Skipped with a console note when
        // the board isn't a winning solution — there are no arrivals to measure,
        // so the existing due is kept and the export still proceeds.
        match stellwerk_sim::suggest_dues(&active.level, &editor.layout, stellwerk_sim::DUE_SLACK_PCT)
        {
            Ok(dues) => {
                for (entry, due) in active.level.schedule.iter_mut().zip(&dues) {
                    entry.due = *due;
                }
                log.info(format!(
                    "Sollzeiten kalibriert (Slack {}%)",
                    stellwerk_sim::DUE_SLACK_PCT
                ));
            }
            Err(e) => log.warn(format!("Sollzeiten nicht kalibriert: {e}")),
        }
        let code = stellwerk_codes::encode(&Payload::Level {
            level: active.level.clone(),
        });
        // Straight to the console (the global tracing bridge would otherwise
        // double these); localized, with the file path kept on the fallback.
        match crate::clipboard::copy(&code) {
            CopyOutcome::Clipboard => log.info(t("console.export_ok")),
            CopyOutcome::File(path) => {
                log.info(format!("{} → {}", t("console.export_ok"), path.display()))
            }
            CopyOutcome::Failed(e) => log.warn(format!("{}: {e}", t("console.export_failed"))),
        }
        // Only persist to the sandbox file when this IS the real sandbox. A dev
        // tweaking a campaign level via "open in sandbox" carries the campaign
        // id, and must not clobber the player's actual sandbox save.
        if active.id == SANDBOX_ID {
            save_sandbox(&active.level);
        }
    }
}
