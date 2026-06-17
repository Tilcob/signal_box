//! Maps the visible buffer slice onto the fixed row pool, in place. Cheap on
//! idle frames: a `(len, offset)` compare short-circuits when nothing moved.

use bevy::prelude::*;

use super::{ConsoleRow, ConsoleView, LOG_INFO, ROWS, RowJump};
use crate::console::{ConsoleLog, Severity};

const LOG_ERROR: Color = Color::srgb(1.0, 0.45, 0.35);
const LOG_WARN: Color = Color::srgb(1.0, 0.78, 0.35);

pub(super) fn console_render(
    log: Res<ConsoleLog>,
    mut view: ResMut<ConsoleView>,
    mut rows: Query<(&ConsoleRow, &mut Text, &mut TextColor, &mut RowJump)>,
    mut last: Local<Option<(usize, usize)>>,
) {
    if view.stick {
        view.offset = log.lines().len().saturating_sub(ROWS);
    }
    let key = (log.lines().len(), view.offset);
    if *last == Some(key) && !log.is_changed() {
        return;
    }
    *last = Some(key);

    let lines = log.lines();
    for (row, mut text, mut color, mut jump) in &mut rows {
        match lines.get(view.offset + row.0) {
            Some(line) => {
                if text.0 != line.text {
                    text.0 = line.text.clone();
                }
                let c = severity_color(line.severity);
                if color.0 != c {
                    color.0 = c;
                }
                // Set every pass (independent of the text guard) so a click after
                // scrolling jumps to the line actually shown, not a stale one.
                if jump.0 != line.jump {
                    jump.0 = line.jump;
                }
            }
            None => {
                if !text.0.is_empty() {
                    text.0.clear();
                }
                if jump.0.is_some() {
                    jump.0 = None;
                }
            }
        }
    }
}

fn severity_color(severity: Severity) -> Color {
    match severity {
        Severity::Error => LOG_ERROR,
        Severity::Warn => LOG_WARN,
        Severity::Info => LOG_INFO,
    }
}
