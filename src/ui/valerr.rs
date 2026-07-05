//! Localized validation-error text. The sim crate's `Display` stays English
//! (logs/tests); the player-facing text is translated here, with the concrete
//! cell/id appended in Rust (the i18n shim has no placeholders). Same split as
//! `select::actions::decode_error_text` for `DecodeError`.

use stellwerk_sim::{Unreachable, Unreached, ValidationError};

use crate::editor::BuildIssue;
use crate::i18n::t;

/// Keys [`build_issue_text`] can emit — kept in sync with the match below for
/// the i18n coverage checker (see `crate::i18n` tests).
#[cfg(test)]
pub(crate) const BUILD_ISSUE_KEYS: &[&str] = &[
    "build.no_source_no_sink",
    "build.no_source",
    "build.no_sink",
];

/// Localized line for a sandbox [`BuildIssue`] (no concrete cell to append).
pub(crate) fn build_issue_text(issue: &BuildIssue) -> String {
    match issue {
        BuildIssue::NothingPlaced => t("build.no_source_no_sink"),
        BuildIssue::MissingSource => t("build.no_source"),
        BuildIssue::MissingSink => t("build.no_sink"),
    }
}

/// Localized reachability-warning line. Shared by the HUD diagnostics panel and
/// the console mirror so they cannot drift. Freight gets its own line: the train
/// reaches its sink but never crosses its assigned platform.
pub(crate) fn unreachable_text(unreachable: &Unreachable) -> String {
    match unreachable.reason {
        Unreached::Sink(_) => format!("{}{}", t("edit.unreachable"), unreachable.train.0),
        Unreached::Platform(platform) => format!(
            "{}{} ({})",
            t("edit.unreachable_platform"),
            unreachable.train.0,
            platform.0
        ),
    }
}

/// Every key [`valerr_text`] can emit — kept beside the match so the i18n
/// coverage checker (see `crate::i18n` tests) can assert all of them resolve in
/// both languages. MUST stay in sync with the arms below; adding a
/// [`ValidationError`] variant breaks the exhaustive match and reminds you.
#[cfg(test)]
pub(crate) const VALERR_KEYS: &[&str] = &[
    "valerr.illegal_pair",
    "valerr.duplicate_piece",
    "valerr.switch_clash",
    "valerr.switch_angle",
    "valerr.switch_default",
    "valerr.switch_rule_branch",
    "valerr.switch_rule_sink",
    "valerr.switch_not_exclusive",
    "valerr.duplicate_switch",
    "valerr.signal_off_track",
    "valerr.duplicate_signal",
    "valerr.junction_no_switch",
    "valerr.connector_reused",
    "valerr.outside_buildable",
    "valerr.dup_source_id",
    "valerr.dup_sink_id",
    "valerr.source_off_track",
    "valerr.sink_off_track",
    "valerr.dup_platform_id",
    "valerr.platform_off_track",
    "valerr.unknown_platform",
    "valerr.dup_train_id",
    "valerr.unknown_source",
    "valerr.unknown_sink",
    "valerr.non_positive_length",
    "valerr.non_positive_speed",
    "valerr.speed_too_high",
    "valerr.due_before_depart",
];

/// Localized validation-error line, with the concrete cell/id appended.
pub(crate) fn valerr_text(error: &ValidationError) -> String {
    use ValidationError::*;
    let at = |cell: &stellwerk_sim::grid::Cell| format!("({}, {})", cell.x, cell.y);
    match error {
        IllegalPiecePair { cell, .. } => format!("{} {}", t("valerr.illegal_pair"), at(cell)),
        DuplicatePiece { cell, .. } => format!("{} {}", t("valerr.duplicate_piece"), at(cell)),
        SwitchConnectorClash { cell } => format!("{} {}", t("valerr.switch_clash"), at(cell)),
        SwitchBranchAngle { cell, .. } => format!("{} {}", t("valerr.switch_angle"), at(cell)),
        SwitchDefaultOutOfRange { cell } => format!("{} {}", t("valerr.switch_default"), at(cell)),
        SwitchRuleBranchOutOfRange { cell } => {
            format!("{} {}", t("valerr.switch_rule_branch"), at(cell))
        }
        SwitchRuleUnknownSink { cell, .. } => {
            format!("{} {}", t("valerr.switch_rule_sink"), at(cell))
        }
        SwitchCellNotExclusive { cell } => {
            format!("{} {}", t("valerr.switch_not_exclusive"), at(cell))
        }
        DuplicateSwitch { cell } => format!("{} {}", t("valerr.duplicate_switch"), at(cell)),
        SignalOffTrack { cell, .. } => format!("{} {}", t("valerr.signal_off_track"), at(cell)),
        DuplicateSignal { cell, .. } => format!("{} {}", t("valerr.duplicate_signal"), at(cell)),
        JunctionWithoutSwitch { point } => {
            format!("{} ({}, {})", t("valerr.junction_no_switch"), point.x, point.y)
        }
        ConnectorReused { cell, .. } => format!("{} {}", t("valerr.connector_reused"), at(cell)),
        OutsideBuildable { cell } => format!("{} {}", t("valerr.outside_buildable"), at(cell)),
        DuplicateSourceId { id } => format!("{} {}", t("valerr.dup_source_id"), id.0),
        DuplicateSinkId { id } => format!("{} {}", t("valerr.dup_sink_id"), id.0),
        SourceOffTrack { id } => format!("{} {}", t("valerr.source_off_track"), id.0),
        SinkOffTrack { id } => format!("{} {}", t("valerr.sink_off_track"), id.0),
        DuplicatePlatformId { id } => format!("{} {}", t("valerr.dup_platform_id"), id.0),
        PlatformOffTrack { id } => format!("{} {}", t("valerr.platform_off_track"), id.0),
        UnknownPlatform { train, platform } => {
            format!("{} {} ({})", t("valerr.unknown_platform"), train.0, platform.0)
        }
        DuplicateTrainId { train } => format!("{} {}", t("valerr.dup_train_id"), train.0),
        UnknownSource { train, source } => {
            format!("{} {} ({})", t("valerr.unknown_source"), train.0, source.0)
        }
        UnknownSink { train, sink } => {
            format!("{} {} ({})", t("valerr.unknown_sink"), train.0, sink.0)
        }
        NonPositiveLength { train } => format!("{} {}", t("valerr.non_positive_length"), train.0),
        NonPositiveSpeed { train } => format!("{} {}", t("valerr.non_positive_speed"), train.0),
        SpeedTooHigh { train } => format!("{} {}", t("valerr.speed_too_high"), train.0),
        DueBeforeDepart { train } => format!("{} {}", t("valerr.due_before_depart"), train.0),
    }
}
