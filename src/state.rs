//! App state machine and the shared resources of the slice.
//!
//! Mode switching is the Zachlike contract (GDD §5): Edit and Run are
//! strictly separate states; Result shows the outcome and leads back.

use bevy::prelude::*;
use stellwerk_sim::Outcome;
use stellwerk_sim::layout::Layout;
use stellwerk_sim::level::Level;

#[derive(States, Default, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GameState {
    #[default]
    LevelSelect,
    Edit,
    Run,
    Result,
}

/// The level being played, plus its catalog identity. In the sandbox the
/// level itself is editable (sources, sinks, schedule — M2 plan §2.2).
#[derive(Resource, Clone)]
pub struct ActiveLevel {
    pub id: String,
    pub index: usize,
    pub level: Level,
    pub sandbox: bool,
}

/// Outcome of the last run, displayed by the Result overlay.
#[derive(Resource, Clone)]
pub struct LastOutcome(pub Outcome);

/// The player's build, with full undo/redo (plan M1 §2: every build action
/// is an invertible operation — the same op vocabulary carries the M2
/// sharing format).
#[derive(Resource, Default)]
pub struct Editor {
    pub layout: Layout,
    pub undo: Vec<crate::editor::EditOp>,
    pub redo: Vec<crate::editor::EditOp>,
    pub tool: Tool,
    /// R-cycled placement variant per tool.
    pub variant: usize,
    /// Cells visited by the current track drag.
    pub drag: Option<Vec<stellwerk_sim::grid::Cell>>,
    /// Switch cell whose config panel is open.
    pub selected_switch: Option<stellwerk_sim::grid::Cell>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Tool {
    #[default]
    Select,
    Track,
    Switch,
    SignalBlock,
    SignalChain,
    Erase,
    /// Sandbox only: place a source / sink on a connector.
    Source,
    Sink,
}

/// Live validation + reachability results for the current build.
#[derive(Resource, Default)]
pub struct Diagnostics {
    pub errors: Vec<stellwerk_sim::ValidationError>,
    pub unreachable: Vec<stellwerk_sim::Unreachable>,
}

impl Diagnostics {
    pub fn start_allowed(&self) -> bool {
        self.errors.is_empty()
    }
}

pub struct StatePlugin;

impl Plugin for StatePlugin {
    fn build(&self, app: &mut App) {
        app.init_state::<GameState>()
            .init_resource::<Editor>()
            .init_resource::<Diagnostics>();
    }
}
