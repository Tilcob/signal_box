//! Live validation: full error report plus reachability warnings, refreshed
//! whenever the build or the level changes.

use bevy::prelude::*;
use stellwerk_sim::{Level, check_reachability, validate};

use crate::state::{ActiveLevel, BuildIssue, Diagnostics, Editor};

pub(super) fn revalidate(
    active: Option<Res<ActiveLevel>>,
    editor: Res<Editor>,
    mut diagnostics: ResMut<Diagnostics>,
) {
    let Some(active) = active else { return };
    if !editor.is_changed() && !active.is_changed() {
        return;
    }
    diagnostics.errors = validate(&active.level, &editor.layout);
    diagnostics.unreachable = if diagnostics.errors.is_empty() {
        check_reachability(&active.level, &editor.layout).unwrap_or_default()
    } else {
        Vec::new()
    };
    // Campaign levels are authored with both; only the sandbox can lack them.
    diagnostics.build_issues = if active.sandbox {
        sandbox_build_issues(&active.level)
    } else {
        Vec::new()
    };
}

/// The sandbox's "needs a source and a sink" rule. A level missing either can
/// never produce a run, so START is blocked until both are placed.
fn sandbox_build_issues(level: &Level) -> Vec<BuildIssue> {
    build_issues_for(level.sources.is_empty(), level.sinks.is_empty())
}

/// Pure decision split out so the three cases are unit-testable without
/// constructing a `Level`.
fn build_issues_for(no_source: bool, no_sink: bool) -> Vec<BuildIssue> {
    match (no_source, no_sink) {
        (true, true) => vec![BuildIssue::NothingPlaced],
        (true, false) => vec![BuildIssue::MissingSource],
        (false, true) => vec![BuildIssue::MissingSink],
        (false, false) => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_issues_cover_all_cases() {
        assert_eq!(build_issues_for(true, true), vec![BuildIssue::NothingPlaced]);
        assert_eq!(build_issues_for(true, false), vec![BuildIssue::MissingSource]);
        assert_eq!(build_issues_for(false, true), vec![BuildIssue::MissingSink]);
        assert!(build_issues_for(false, false).is_empty());
    }
}
