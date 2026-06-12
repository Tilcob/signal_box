//! Game state machine + the single system set all gameplay systems live in.

use bevy::prelude::*;

/// Top-level state machine.
#[derive(States, Default, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GameState {
    /// The timetable is running (or paused).
    #[default]
    Playing,
    /// A run finished — either by collision or by completing the timetable.
    /// See the [`crate::sim::EndCause`] resource for which one it was.
    Ended,
}

/// Pause as a *sub-state* of [`GameState::Playing`]: resuming does not
/// re-enter `Playing`, so `OnEnter(GameState::Playing)` systems only fire when
/// a session genuinely (re)starts — never on unpause.
#[derive(SubStates, Default, Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[source(GameState = GameState::Playing)]
pub enum RunState {
    #[default]
    Running,
    Paused,
}

/// Every simulation system goes `.in_set(GameplaySet)`. The "only while
/// playing and not paused" gate is configured once here instead of repeated
/// per system, so a new system can't silently forget the pause check.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GameplaySet;

pub struct GameStatePlugin;

impl Plugin for GameStatePlugin {
    fn build(&self, app: &mut App) {
        app.init_state::<GameState>()
            .add_sub_state::<RunState>()
            // `in_state(RunState::Running)` is false while the sub-state does
            // not exist (i.e. outside `Playing`), so this one condition covers
            // both "in Playing" and "not paused".
            .configure_sets(Update, GameplaySet.run_if(in_state(RunState::Running)))
            .add_systems(Update, toggle_pause.run_if(in_state(GameState::Playing)));
    }
}

fn toggle_pause(
    input: Res<ButtonInput<KeyCode>>,
    current: Res<State<RunState>>,
    mut next: ResMut<NextState<RunState>>,
) {
    if input.just_pressed(KeyCode::Escape) {
        next.set(match current.get() {
            RunState::Running => RunState::Paused,
            RunState::Paused => RunState::Running,
        });
    }
}
