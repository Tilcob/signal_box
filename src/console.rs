//! In-level message console: a capped ring buffer of player-facing lines
//! (build errors, run outcomes, level exports). Data + API + the `tracing`
//! bridge; the panel that renders it lives in `crate::ui::console`.
//!
//! Two sources feed it: explicit [`ConsoleLog::info`]/[`warn`](ConsoleLog::warn)/
//! [`error`](ConsoleLog::error) calls, and a tracing layer ([`console_layer`],
//! wired via `LogPlugin::custom_layer`) that mirrors OUR crates' `info!`/
//! `warn!`/`error!` — including dev — into the log. Engine noise (wgpu/winit/
//! bevy internals) is filtered out by target, so the console still shows only
//! our own messages, never the engine firehose.

use bevy::log::BoxedLayer;
use bevy::log::tracing::field::{Field, Visit};
use bevy::log::tracing::{Event, Level, Subscriber};
use bevy::log::tracing_subscriber::Layer;
use bevy::log::tracing_subscriber::layer::Context;
use bevy::prelude::*;
use std::collections::VecDeque;
use std::fmt::Write as _;
use std::sync::Mutex;

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
    /// World position to recentre the camera on when the line is clicked, for
    /// located diagnostics (a board cell/connector). `None` = not clickable.
    pub jump: Option<Vec2>,
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
        self.push_at(severity, text, None);
    }

    /// Like [`push`](Self::push) but carries a camera jump target — for located
    /// diagnostics whose console line should recentre the board when clicked.
    pub fn push_at(&mut self, severity: Severity, text: impl Into<String>, jump: Option<Vec2>) {
        if self.lines.len() == CAP {
            self.lines.pop_front();
        }
        self.lines.push_back(ConsoleLine {
            severity,
            text: text.into(),
            jump,
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

// --- tracing → console bridge --------------------------------------------------

/// Hand-off from the tracing layer (called on whatever thread logs, possibly
/// before the app loops) to the drain system. A plain global because the layer
/// is installed in `LogPlugin` and has no access to the ECS world.
static BRIDGE_QUEUE: Mutex<Vec<(Severity, String)>> = Mutex::new(Vec::new());

/// Tracing layer that captures OUR crates' events into [`BRIDGE_QUEUE`]. Engine
/// targets (wgpu/winit/bevy_*) are skipped, so the console stays our-messages-
/// only. Mapped: ERROR→Error, WARN→Warn, everything else→Info.
struct ConsoleBridge;

impl<S: Subscriber> Layer<S> for ConsoleBridge {
    fn on_event(&self, event: &Event<'_>, _ctx: Context<'_, S>) {
        let target = event.metadata().target();
        if !target.starts_with("signal_box") && !target.starts_with("stellwerk") {
            return;
        }
        let mut visitor = MessageVisitor(String::new());
        event.record(&mut visitor);
        if visitor.0.is_empty() {
            return;
        }
        let severity = match *event.metadata().level() {
            Level::ERROR => Severity::Error,
            Level::WARN => Severity::Warn,
            _ => Severity::Info,
        };
        if let Ok(mut queue) = BRIDGE_QUEUE.lock() {
            queue.push((severity, visitor.0));
        }
    }
}

/// Pulls the formatted text out of an event's `message` field.
struct MessageVisitor(String);

impl Visit for MessageVisitor {
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        if field.name() == "message" {
            let _ = write!(self.0, "{value:?}");
        }
    }
}

/// The layer for `LogPlugin::custom_layer` (a plain `fn` to match its pointer
/// signature). See the module docs.
pub fn console_layer(_app: &mut App) -> Option<BoxedLayer> {
    Some(Box::new(ConsoleBridge))
}

/// Moves captured log events into the console each frame (empty on idle).
fn drain_log_bridge(mut log: ResMut<ConsoleLog>) {
    let Ok(mut queue) = BRIDGE_QUEUE.lock() else {
        return;
    };
    for (severity, text) in queue.drain(..) {
        log.push(severity, text);
    }
}

pub struct ConsolePlugin;

impl Plugin for ConsolePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ConsoleLog>()
            .init_resource::<ConsoleHovered>()
            .add_systems(Update, drain_log_bridge);
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
    fn bridge_captures_our_events_and_skips_engine() {
        use bevy::log::tracing::subscriber::with_default;
        use bevy::log::tracing::{info, warn};
        use bevy::log::tracing_subscriber::prelude::*;

        BRIDGE_QUEUE.lock().unwrap().clear();
        // `with_default` installs the layer thread-locally — no global default,
        // so this is safe alongside the rest of the suite.
        let subscriber = bevy::log::tracing_subscriber::registry().with(ConsoleBridge);
        with_default(subscriber, || {
            info!(target: "signal_box::probe", "hello {}", 42);
            warn!(target: "wgpu::core", "engine noise to ignore");
        });
        let captured: Vec<_> = BRIDGE_QUEUE.lock().unwrap().drain(..).collect();
        assert_eq!(captured.len(), 1, "only our target is captured, not wgpu");
        assert_eq!(captured[0].1, "hello 42", "message extracted, no quotes");
        assert!(matches!(captured[0].0, Severity::Info));
    }

    #[test]
    fn clamp_offset_edges() {
        assert_eq!(clamp_offset(0, 0, 12), 0); // empty buffer
        assert_eq!(clamp_offset(5, 3, 12), 0); // fewer lines than rows
        assert_eq!(clamp_offset(100, 20, 12), 8); // clamp to len - rows
        assert_eq!(clamp_offset(4, 20, 12), 4); // already in range
    }
}
