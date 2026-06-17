//! In-level message console: a capped ring buffer the app writes player-facing
//! lines into (build errors, run outcomes, level exports). Data + API only —
//! the panel that renders it lives in `crate::ui::console`. The app curates
//! what lands here on purpose; this is not a `tracing` sink, so engine noise
//! (wgpu/winit) never reaches the player.

use bevy::prelude::*;
use std::collections::VecDeque;

/// Line severity, ranked low→high. The UI maps these to colors (error red,
/// warn orange, info white); this module stays color-agnostic.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Info,
    Warn,
    Error,
}

pub struct ConsoleLine {
    pub severity: Severity,
    pub text: String,
}

/// Retained-line cap; the oldest drop first.
const CAP: usize = 500; // ponytail: 500 lines, raise if the history feels short.

/// The app's player-facing log: a capped ring, newest at the back.
#[derive(Resource, Default)]
pub struct ConsoleLog {
    lines: VecDeque<ConsoleLine>,
}

impl ConsoleLog {
    pub fn push(&mut self, severity: Severity, text: impl Into<String>) {
        if self.lines.len() == CAP {
            self.lines.pop_front();
        }
        self.lines.push_back(ConsoleLine {
            severity,
            text: text.into(),
        });
    }

    pub fn error(&mut self, text: impl Into<String>) {
        self.push(Severity::Error, text);
    }
    pub fn warn(&mut self, text: impl Into<String>) {
        self.push(Severity::Warn, text);
    }
    pub fn info(&mut self, text: impl Into<String>) {
        self.push(Severity::Info, text);
    }

    pub fn lines(&self) -> &VecDeque<ConsoleLine> {
        &self.lines
    }
}

/// Set each frame by the console UI from its root's `Interaction`; read by
/// `camera::zoom` to suppress wheel-zoom while the pointer is over the console
/// (otherwise the same wheel events both scroll the log and zoom the board).
#[derive(Resource, Default)]
pub struct ConsoleHovered(pub bool);

/// Clamp a scroll offset so the visible window `[offset, offset + rows)` stays
/// inside `len`. Pure, to unit-test the off-by-one edges without an app.
pub fn clamp_offset(offset: usize, len: usize, rows: usize) -> usize {
    offset.min(len.saturating_sub(rows))
}

pub struct ConsolePlugin;

impl Plugin for ConsolePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ConsoleLog>()
            .init_resource::<ConsoleHovered>();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ring_evicts_oldest_and_keeps_order() {
        let mut log = ConsoleLog::default();
        for i in 0..(CAP + 3) {
            log.info(i.to_string());
        }
        assert_eq!(log.lines().len(), CAP);
        // The first three pushes (0, 1, 2) were evicted.
        assert_eq!(log.lines().front().unwrap().text, "3");
        assert_eq!(log.lines().back().unwrap().text, (CAP + 2).to_string());
    }

    #[test]
    fn clamp_offset_edges() {
        assert_eq!(clamp_offset(0, 0, 12), 0); // empty buffer
        assert_eq!(clamp_offset(5, 3, 12), 0); // fewer lines than rows
        assert_eq!(clamp_offset(100, 20, 12), 8); // clamp to len - rows
        assert_eq!(clamp_offset(4, 20, 12), 4); // already in range
    }
}
