//! Ctrl + mouse wheel: cycles the track ghost's exit connector through the legal
//! curve forms.

use bevy::input::mouse::{MouseScrollUnit, MouseWheel};
use bevy::prelude::*;
use stellwerk_sim::grid::{Dir8, pair_len};

use crate::state::{Editor, Tool};

/// Ctrl + mouse wheel cycles the track ghost's exit connector through the legal
/// curve forms (entry fixed) — the always-visible ring in `overlays` shows the
/// options. Without Ctrl the wheel is the camera zoom (`camera::zoom` yields
/// while Ctrl is held). Bypasses change detection like the other tool inputs so
/// a notch never rebuilds the board.
pub(crate) fn cycle_track_form(
    keys: Res<ButtonInput<KeyCode>>,
    mut wheel: MessageReader<MouseWheel>,
    mut editor: ResMut<Editor>,
) {
    let ctrl = keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight);
    let act = ctrl && editor.tool == Tool::Track;
    // Always drain the wheel, so a notch scrolled while inactive can't fire late.
    let mut notches = 0i32;
    for event in wheel.read() {
        if act {
            // Native reports Line units (±1/notch), browsers Pixel (~100/notch).
            notches += match event.unit {
                MouseScrollUnit::Line => event.y.round() as i32,
                MouseScrollUnit::Pixel => (event.y / 100.0).round() as i32,
            };
        }
    }
    if notches == 0 {
        return;
    }
    // Wheel up walks toward the "up" curves first (gentle-up, sharp-up, then
    // sharp-down, gentle-down): negative rotation around the compass. Wheel down
    // reverses.
    let step = if notches > 0 { -1 } else { 1 };
    let bypass = editor.bypass_change_detection();
    let (entry, mut exit) = bypass.track_form;
    for _ in 0..notches.unsigned_abs() {
        exit = next_exit(entry, exit, step);
    }
    bypass.track_form = (entry, exit);
}

/// The next legal exit connector from `entry`, walking `step` (±1) around the
/// compass and skipping the entry itself and kinks (`pair_len` rejects). Wraps;
/// returns `exit` unchanged when the cell admits no other legal form.
fn next_exit(entry: Dir8, exit: Dir8, step: i32) -> Dir8 {
    let mut d = exit;
    for _ in 0..8 {
        d = d.rotate(step);
        if d != entry && pair_len(entry, d).is_some() {
            return d;
        }
    }
    exit
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The Ctrl+wheel cycle never lands on an illegal exit: not the entry, not a
    /// kink, and it actually moves (so it can reach curves, not just spin in
    /// place).
    #[test]
    fn next_exit_stays_legal_and_moves() {
        let entry = Dir8::W;
        let mut d = Dir8::E;
        for step in [1, -1] {
            for _ in 0..16 {
                let n = next_exit(entry, d, step);
                assert_ne!(n, entry, "exit never equals the entry");
                assert!(pair_len(entry, n).is_some(), "exit is never a kink");
                d = n;
            }
        }
        let a = next_exit(Dir8::W, Dir8::E, 1);
        let b = next_exit(Dir8::W, a, 1);
        assert_ne!(a, b, "consecutive steps advance through the legal set");
    }
}
