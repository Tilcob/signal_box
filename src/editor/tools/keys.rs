//! Keyboard: tool selection (1–7, Q, B), R/T rotation and layout-aware undo/redo.

use bevy::input::keyboard::Key;
use bevy::prelude::*;

use crate::editor::ops::{redo, undo};
use crate::state::{ActiveLevel, Editor, Tool};

pub(crate) fn hotkeys(
    keys: Res<ButtonInput<KeyCode>>,
    logical: Res<ButtonInput<Key>>,
    active: Option<ResMut<ActiveLevel>>,
    mut editor: ResMut<Editor>,
) {
    let mut active = active;
    let sandbox = active.as_ref().is_some_and(|a| a.sandbox);
    let bypass = editor.bypass_change_detection();
    if keys.just_pressed(KeyCode::KeyQ) {
        bypass.tool = Tool::Select;
    }
    if keys.just_pressed(KeyCode::Digit1) {
        bypass.tool = Tool::Track;
    }
    if keys.just_pressed(KeyCode::Digit2) {
        bypass.tool = Tool::Switch;
    }
    if keys.just_pressed(KeyCode::Digit3) {
        bypass.tool = Tool::SignalBlock;
    }
    if keys.just_pressed(KeyCode::Digit4) {
        bypass.tool = Tool::SignalChain;
    }
    if keys.just_pressed(KeyCode::KeyB) {
        bypass.tool = Tool::Erase;
    }
    if sandbox && keys.just_pressed(KeyCode::Digit5) {
        bypass.tool = Tool::Block;
    }
    if sandbox && keys.just_pressed(KeyCode::Digit6) {
        bypass.tool = Tool::Source;
    }
    if sandbox && keys.just_pressed(KeyCode::Digit7) {
        bypass.tool = Tool::Sink;
    }
    // R = rotate left (−45°), T = rotate right (+45°) for every tool. Tracks
    // rotate their whole form through the 8 orientations; switch/signal rotate
    // their variant counter.
    let r = keys.just_pressed(KeyCode::KeyR);
    let t = keys.just_pressed(KeyCode::KeyT);
    if r ^ t {
        let steps = if r { -1 } else { 1 };
        match bypass.tool {
            Tool::Track => {
                let (a, b) = bypass.track_form;
                bypass.track_form = (a.rotate(steps), b.rotate(steps));
            }
            _ => bypass.variant += steps,
        }
    }

    // Undo/redo match the LOGICAL key, not the physical KeyCode: KeyCode
    // names US positions, so on a German QWERTZ layout the key labeled "Z"
    // arrives as KeyCode::KeyY — Ctrl+Z would silently trigger redo.
    let chr = |s: &str| Key::Character(s.into());
    let ctrl = keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight);
    let undo_pressed = logical.just_pressed(chr("z")) || logical.just_pressed(chr("Z"));
    let redo_pressed = logical.just_pressed(chr("y")) || logical.just_pressed(chr("Y"));
    // undo/redo replay layout AND sandbox level ops, so they need the level.
    // `&mut editor`/`&mut active.level` re-borrow WITH change detection on
    // purpose: the merged layout and schedule panel must rebuild afterwards.
    if ctrl && undo_pressed && let Some(active) = active.as_deref_mut() {
        undo(&mut editor, &mut active.level);
    }
    if ctrl && redo_pressed && let Some(active) = active.as_deref_mut() {
        redo(&mut editor, &mut active.level);
    }
}
