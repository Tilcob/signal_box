//! The train simulation: track graph, signals, trains, timetable, scoring.

pub mod graph;
pub mod signal;
pub mod train;

pub use graph::{NodeKind, TrackGraph};
pub use signal::Signal;
pub use train::{Score, Timetable, Train};

use bevy::prelude::*;

use crate::core::GameplaySet;

/// Why the last run ended. Inserted when [`crate::core::GameState::Ended`] is
/// entered, removed on restart.
#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq)]
pub enum EndCause {
    Crash,
    Completed,
}

pub struct SimPlugin;

impl Plugin for SimPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(graph::build_level())
            .insert_resource(train::fresh_timetable())
            .init_resource::<Score>()
            .add_systems(Startup, signal::spawn_signals)
            .add_systems(
                Update,
                (
                    train::spawn_departures,
                    train::move_trains,
                    train::detect_collisions,
                    train::check_completion,
                )
                    .chain()
                    .in_set(GameplaySet),
            )
            .add_systems(Update, signal::tint_signal_sprites);
    }
}
