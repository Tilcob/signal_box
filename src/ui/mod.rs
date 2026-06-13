//! bevy_ui screens, one plugin per screen: level select (with sandbox, code
//! import, language toggle), edit HUD (solution slots, start button), switch
//! config panel, schedule panel, run HUD and the result overlay with code
//! export (M2 plan §2). Shared theme + widget helpers live in [`widgets`].

pub(crate) mod edit_hud;
mod main_menu;
mod result;
mod run_hud;
mod schedule_panel;
mod select;
mod switch_panel;
mod widgets;

use bevy::prelude::*;

use crate::levels::Progress;
use crate::state::{ActiveLevel, Editor, GameState, Tool};

pub struct UiPlugin;

impl Plugin for UiPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            main_menu::MainMenuPlugin,
            select::SelectUiPlugin,
            edit_hud::EditHudPlugin,
            switch_panel::SwitchPanelPlugin,
            schedule_panel::SchedulePanelPlugin,
            run_hud::RunHudPlugin,
            result::ResultPlugin,
        ))
        // All states: hover/press feedback for every button.
        .add_systems(Update, widgets::button_feedback);
    }
}

/// Loads a level into a fresh editor session and switches to Edit — used by
/// the level select and the result screen's "next level" button.
#[allow(clippy::too_many_arguments)]
fn enter_level(
    index: usize,
    id: String,
    level: stellwerk_sim::Level,
    sandbox: bool,
    progress: &Progress,
    commands: &mut Commands,
    editor: &mut Editor,
    next: &mut NextState<GameState>,
) {
    editor.layout = progress
        .levels
        .get(&id)
        .map(|p| p.layout.clone())
        .unwrap_or_default();
    editor.undo.clear();
    editor.redo.clear();
    editor.tool = Tool::Track;
    editor.variant = 0;
    editor.selected_switch = None;
    editor.drag = None;
    commands.insert_resource(ActiveLevel {
        id,
        index,
        level,
        sandbox,
    });
    next.set(GameState::Edit);
}
