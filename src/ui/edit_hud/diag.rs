//! Mirrors the live build diagnostics into the in-level console as an event
//! journal — appearances and resolutions are logged as discrete lines, each
//! with a camera-jump target where one is resolvable. Spawns nothing; pure
//! logic over `Diagnostics` and the console log.

use bevy::prelude::*;
use std::collections::HashSet;

use crate::console::{ConsoleLog, Severity};
use crate::editor::Diagnostics;
use crate::i18n::t;
use crate::state::{ActiveLevel, Editor, GameState};
use crate::ui::valerr::{build_issue_text, unreachable_text, valerr_text};

pub(super) struct DiagPlugin;

impl Plugin for DiagPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<LastDiag>()
            .add_systems(OnEnter(GameState::Edit), clear_last_diag)
            .add_systems(
                Update,
                mirror_diagnostics_to_console
                    .run_if(resource_changed::<Diagnostics>)
                    .run_if(in_state(GameState::Edit)),
            );
    }
}

/// Diagnostic lines as of the last console mirror, by text. The mirror diffs the
/// current diagnostics against this to log appearances and resolutions as
/// discrete events. Cleared on entering Edit (a different level starts fresh).
#[derive(Resource, Default)]
struct LastDiag(HashSet<String>);

/// Reset the diff set on entering Edit — the HUD/level may have changed, so the
/// next diff logs from scratch. (Was part of `fill_edit_texts`.)
fn clear_last_diag(mut last: ResMut<LastDiag>) {
    last.0.clear();
}

/// World position to recentre on for a located error, or `None` for the
/// schedule/id errors that have no single board cell. The source/sink-off-track
/// errors carry only an id, so they need the `level` to resolve their cell.
/// Exhaustive on purpose — a new [`stellwerk_sim::ValidationError`] variant must
/// be classified here.
fn error_world(
    error: &stellwerk_sim::ValidationError,
    level: Option<&stellwerk_sim::Level>,
) -> Option<Vec2> {
    use stellwerk_sim::ValidationError::*;
    let cell = match error {
        IllegalPiecePair { cell, .. }
        | DuplicatePiece { cell, .. }
        | SwitchConnectorClash { cell }
        | SwitchBranchAngle { cell, .. }
        | SwitchDefaultOutOfRange { cell }
        | SwitchRuleBranchOutOfRange { cell }
        | SwitchRuleUnknownSink { cell, .. }
        | SwitchCellNotExclusive { cell }
        | DuplicateSwitch { cell }
        | SignalOffTrack { cell, .. }
        | DuplicateSignal { cell, .. }
        | ConnectorReused { cell, .. }
        | OutsideBuildable { cell } => *cell,
        JunctionWithoutSwitch { point } => return Some(crate::board::point_world(*point)),
        SourceOffTrack { id } => level?.sources.iter().find(|s| s.id == *id)?.cell,
        SinkOffTrack { id } => level?.sinks.iter().find(|s| s.id == *id)?.cell,
        DuplicateSourceId { .. }
        | DuplicateSinkId { .. }
        | DuplicateTrainId { .. }
        | UnknownSource { .. }
        | UnknownSink { .. }
        | NonPositiveLength { .. }
        | NonPositiveSpeed { .. }
        | SpeedTooHigh { .. }
        | DueBeforeDepart { .. } => return None,
    };
    Some(crate::board::cell_world(cell))
}

/// World position for a reachability warning: the cell of the train's source
/// (resolved through the schedule), so the console line can recentre there.
fn unreachable_world(
    unreachable: &stellwerk_sim::Unreachable,
    level: Option<&stellwerk_sim::Level>,
) -> Option<Vec2> {
    let level = level?;
    let entry = level
        .schedule
        .iter()
        .find(|e| e.train == unreachable.train)?;
    let source = level.sources.iter().find(|s| s.id == entry.source)?;
    Some(crate::board::cell_world(source.cell))
}

/// Mirrors diagnostics into the in-level console as an event journal: a problem
/// is logged when it appears (errors/build-blocks as errors, reachability as
/// warnings, all with a camera jump where one is resolvable) and an info
/// "resolved" line when it clears. The set is diffed against [`LastDiag`], so a
/// genuinely re-appearing problem logs again — unlike a monotonic de-dup. Logging
/// is skipped while a track drag is in progress (it flaps the diagnostics every
/// cell); the settled state is logged once the drag finishes.
fn mirror_diagnostics_to_console(
    diagnostics: Res<Diagnostics>,
    editor: Res<Editor>,
    active: Option<Res<ActiveLevel>>,
    mut log: ResMut<ConsoleLog>,
    mut last: ResMut<LastDiag>,
) {
    if editor.drag.is_some() {
        return;
    }
    let level = active.as_ref().map(|a| &a.level);
    // Current diagnostics as (text, severity, jump), in display order.
    let mut current: Vec<(String, Severity, Option<Vec2>)> = Vec::new();
    for issue in &diagnostics.build_issues {
        current.push((build_issue_text(issue), Severity::Error, None));
    }
    for error in &diagnostics.errors {
        current.push((valerr_text(error), Severity::Error, error_world(error, level)));
    }
    for unreachable in &diagnostics.unreachable {
        current.push((
            unreachable_text(unreachable),
            Severity::Warn,
            unreachable_world(unreachable, level),
        ));
    }
    let current_texts: HashSet<String> = current.iter().map(|(text, ..)| text.clone()).collect();
    // Newly appeared since last diff → log with its severity and jump target.
    for (text, severity, jump) in &current {
        if !last.0.contains(text) {
            log.push_at(*severity, text.clone(), *jump);
        }
    }
    // Resolved since last diff → a self-correcting "behoben" info line.
    for text in last.0.difference(&current_texts) {
        log.info(format!("{} {text}", t("console.resolved")));
    }
    last.0 = current_texts;
}
