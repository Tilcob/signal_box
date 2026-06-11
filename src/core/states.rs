//! Game state machine + the single system set all gameplay systems live in.

use bevy::prelude::*;
use ldtk_integration::{LdtkLevelReadyEvent, LdtkLoadState, LdtkLoadStatus};

/// How long `Loading` waits for a level before giving up and entering
/// `Playing` without one (fresh template, missing/broken world file).
const LOAD_TIMEOUT_SECS: f32 = 5.0;

/// Top-level state machine. Extend with `MainMenu`, `GameOver`, … as needed.
#[derive(States, Default, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GameState {
    /// Waiting for the LDtk world to load and the first level to become ready.
    #[default]
    Loading,
    Playing,
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

/// Every gameplay system goes `.in_set(GameplaySet)`. The "only while playing
/// and not paused" gate is configured once here instead of repeated per system,
/// so a new system can't silently forget the pause check.
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
            .add_systems(
                Update,
                advance_to_playing.run_if(in_state(GameState::Loading)),
            )
            .add_systems(Update, toggle_pause.run_if(in_state(GameState::Playing)));
    }
}

/// Leaves `Loading` once the first level is ready — or, as a template-friendly
/// fallback, when loading failed / timed out, so the app is never stuck on a
/// missing world file.
fn advance_to_playing(
    mut ready: MessageReader<LdtkLevelReadyEvent>,
    load_state: Res<LdtkLoadState>,
    time: Res<Time>,
    mut waited: Local<f32>,
    mut next: ResMut<NextState<GameState>>,
) {
    if ready.read().next().is_some() {
        next.set(GameState::Playing);
        return;
    }

    if load_state.status == LdtkLoadStatus::Error {
        warn!("LDtk world failed to load — entering Playing without a level.");
        next.set(GameState::Playing);
        return;
    }

    *waited += time.delta_secs();
    if *waited > LOAD_TIMEOUT_SECS {
        warn!(
            "No level became ready after {LOAD_TIMEOUT_SECS}s — entering Playing without one. \
             Is `assets/{}` present?",
            crate::level::WORLD_PATH
        );
        next.set(GameState::Playing);
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
