//! Edit mode: tools, drag track drawing, invertible edit ops with undo/redo,
//! live validation + reachability warnings (plan M1 §2/§3; M2: sandbox
//! source/sink tools).
//!
//! Validation is never modal: faulty elements glow on the board, the start
//! switch stays locked while errors exist. Reachability problems are
//! warnings only — watching the misrouting happen is a lesson (Säule 4).
//!
//! Sandbox note (M2-minimal): source/sink placement and the schedule editor
//! mutate the LEVEL, not the layout — they sit outside the undo stack.

mod ops;
mod overlays;
mod placement;
mod radial;
mod tools;
mod validation;

pub use ops::{EditOp, do_op};

use bevy::prelude::*;

use crate::state::GameState;

pub struct EditorPlugin;

impl Plugin for EditorPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                tools::hotkeys,
                tools::pointer,
                overlays::draw_overlays,
                validation::revalidate,
                tools::leave_to_select,
                // After leave_to_select: it yields to an open menu, so the
                // menu must still be open when it runs — close happens here.
                radial::radial_menu,
            )
                .chain()
                .run_if(in_state(GameState::Edit)),
        );
    }
}
