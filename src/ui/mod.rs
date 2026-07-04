//! bevy_ui screens, one plugin per screen: level select (with sandbox, code
//! import, language toggle), edit HUD (solution slots, start button), switch
//! config panel, schedule panel, run HUD and the result overlay with code
//! export. Shared theme + widget helpers live in [`widgets`].
//!
//! Screen module convention — so one change stays in one place:
//! - A screen owns its own state. Screen-local resources/components/markers
//!   live in the screen module, not in `crate::state` (which holds only what
//!   several screens share). Cross-cutting state read by gameplay systems
//!   (`Paused`, `FocusedField`, `SaveModalOpen`) is the exception and stays in
//!   `crate::state`.
//! - Each screen tags its root entity with its own marker (`UiMainMenu`,
//!   `UiEdit`, …) and despawns it on `OnExit(state)` via
//!   [`widgets::despawn_all`]. Screens that own a toggle resource
//!   (`Paused`, `HelpOpen`) reset it in a hand-written `teardown` instead, so
//!   the resource and its entities clear together (see `pause`, `encyclopedia`).
//! - One file until a screen grows past one concern, then a folder module with
//!   `mod`/`view`/`logic` (see `console`, `edit_hud`, `select`).

#[cfg(feature = "dev")]
mod campaign_save;
mod console;
pub(crate) mod edit_hud;
pub(crate) mod encyclopedia;
mod hints;
mod juice;
mod main_menu;
mod numeric_field;
mod options;
pub(crate) mod pause;
mod result;
mod run_hud;
mod sandbox_setup;
mod schedule_panel;
pub(crate) mod select;
mod station_panel;
mod switch_panel;
mod toolbar;
pub(crate) mod valerr;
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
            sandbox_setup::SandboxSetupUiPlugin,
            edit_hud::EditHudPlugin,
            console::ConsoleUiPlugin,
            switch_panel::SwitchPanelPlugin,
            numeric_field::NumericFieldPlugin,
            schedule_panel::SchedulePanelPlugin,
            station_panel::StationPanelPlugin,
            run_hud::RunHudPlugin,
            result::ResultPlugin,
            encyclopedia::EncyclopediaPlugin,
            pause::PausePlugin,
            toolbar::ToolbarPlugin,
            // Nested so the outer tuple stays within Bevy's 15-element `Plugins`.
            (
                options::OptionsPlugin,
                juice::JuicePlugin,
                hints::HintsPlugin,
            ),
        ))
        // All states: hover/press feedback for every button.
        .add_systems(Update, widgets::button_feedback);
        #[cfg(feature = "dev")]
        app.add_plugins(campaign_save::CampaignSavePlugin);
    }
}

/// Loads a level into a fresh editor session and switches to Edit — used by
/// the level select and the result screen's "next level" button.
#[allow(clippy::too_many_arguments)]
fn enter_level(
    index: usize,
    id: String,
    level: stellwerk_sim::Level,
    briefing: String,
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
    editor.track_form = (stellwerk_sim::grid::Dir8::W, stellwerk_sim::grid::Dir8::E);
    editor.selected_switch = None;
    editor.drag = None;
    editor.radial = None;
    commands.insert_resource(ActiveLevel {
        id,
        index,
        level,
        briefing,
        sandbox,
    });
    next.set(GameState::Edit);
}
