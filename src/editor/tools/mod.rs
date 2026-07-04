//! Tool input, one file per responsibility: keyboard [`keys`], the wheel curve
//! cycle [`wheel`], the board [`pointer`] dispatch, track-drag drawing [`track`],
//! block/erase strokes [`strokes`] and the shared placement-commit helper
//! [`commit`]. The editor plugin registers `hotkeys`, `cycle_track_form` and
//! `pointer`.

mod commit;
mod keys;
mod pointer;
mod strokes;
mod track;
mod wheel;

pub(super) use keys::hotkeys;
pub(super) use pointer::pointer;
pub(super) use wheel::cycle_track_form;
