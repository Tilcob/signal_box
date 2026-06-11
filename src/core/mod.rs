//! App-wide foundations: the game state machine and the hot-reloadable
//! tunables. Everything else builds on top of these.

pub mod states;
pub mod tunables;

// Template API — `GameState`/`RunState` are unused until game code adds its
// first `OnEnter(GameState::Playing)` (or similar) hook.
#[allow(unused_imports)]
pub use states::{GameState, GameplaySet, RunState};
pub use tunables::Tunables;

use bevy::prelude::*;

pub struct CorePlugin;

impl Plugin for CorePlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((states::GameStatePlugin, tunables::TunablesPlugin));
    }
}
