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
