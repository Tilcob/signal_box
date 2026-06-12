//! Board rendering — the Pult look (GDD §10): near-black desk, tracks as
//! narrow light bands, state shown by glow color AND shape (accessibility,
//! GDD §9: occupied bands are wider, reservations narrower; chain signals
//! are diamonds, block signals squares — never color alone).
//!
//! Strategy (M1-minimal): retained sprites, fully rebuilt when the build
//! changes (edit) or every frame (run — states change constantly, boards
//! are small). No lyon yet: every stub is a straight segment in the stub
//! model, so rotated quads cover the whole look. Revisit when curves or
//! round caps are wanted (GDD §12.2 note).

mod draw;
mod edit_board;
mod geometry;
mod palette;
mod run_board;

pub use geometry::{CELL, cell_world, connector_world, nearest_connector, point_world, world_cell};

use bevy::prelude::*;

use crate::state::{ActiveLevel, Editor, GameState};
use draw::{BoardGfx, LiveGfx, despawn_all};

pub struct BoardPlugin;

impl Plugin for BoardPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameState::Edit), edit_board::rebuild_edit_board)
            .add_systems(
                Update,
                edit_board::rebuild_edit_board.run_if(in_state(GameState::Edit).and(
                    resource_changed::<Editor>.or(resource_exists_and_changed::<ActiveLevel>),
                )),
            )
            .add_systems(OnExit(GameState::Edit), despawn_all::<BoardGfx>)
            .add_systems(
                Update,
                run_board::draw_run_board.run_if(in_state(GameState::Run)),
            )
            // Result is a frozen final frame: draw it once on enter instead
            // of despawning + respawning every sprite every frame while
            // nothing changes.
            .add_systems(OnEnter(GameState::Result), run_board::draw_run_board)
            // Cleanup on ENTERING Edit/LevelSelect, not on leaving Result:
            // Esc skips Result entirely (Run → Edit → LevelSelect), and an
            // OnExit(Result)-only despawn leaks frozen trains, labels and
            // bands into the editor — zoomed Text2d ghosts looked like
            // giant corrupted glyphs. Run → Result keeps everything.
            .add_systems(OnEnter(GameState::Edit), despawn_all::<LiveGfx>)
            .add_systems(OnEnter(GameState::LevelSelect), despawn_all::<LiveGfx>);
    }
}
