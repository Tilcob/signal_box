//! Station rename panel (bottom right, sandbox only): one row per source and
//! sink with a text field to rename it. Campaign levels don't show it — naming
//! stations is an authoring concern.
//!
//! Commits mutate the level through `bypass_change_detection`, exactly like the
//! schedule's numeric edits: marking `ActiveLevel` changed would rebuild this
//! panel mid-edit and despawn the field the user just clicked/tabbed into,
//! dropping keyboard focus. The board label still redraws — `do_op` flags the
//! `Editor` resource, and the edit board rebuilds on `Editor` OR `ActiveLevel`
//! changing (see `board::BoardPlugin`).

use bevy::prelude::*;
use stellwerk_sim::units::{SinkId, SourceId};

use super::numeric_field::{TextFieldCommit, text_field};
use super::widgets::{TEXT_BRIGHT, TEXT_DIM, text_bundle};
use crate::editor::{EditOp, do_op};
use crate::font::UiFont;
use crate::i18n::t;
use crate::state::{ActiveLevel, Editor, GameState};

/// Max characters for a station name — wide enough for "Hauptbahnhof", short
/// enough to stay on the board.
const NAME_MAX: usize = 16;

/// Root node, spawned by the edit HUD (it owns the Edit screen layout).
#[derive(Component)]
pub(super) struct StationPanelRoot;

#[derive(Clone, Copy)]
enum StationKind {
    Source,
    Sink,
}

/// Marker on a station name field, mapping its commit back to the station.
#[derive(Component, Clone, Copy)]
struct StationField {
    kind: StationKind,
    id: u32,
}

pub(super) struct StationPanelPlugin;

impl Plugin for StationPanelPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                rebuild_station_panel.run_if(resource_exists_and_changed::<ActiveLevel>),
                station_field_commits,
            )
                .run_if(in_state(GameState::Edit)),
        );
    }
}

pub(super) fn rebuild_station_panel(
    mut commands: Commands,
    ui_font: Res<UiFont>,
    mut roots: Query<(Entity, &mut Node), With<StationPanelRoot>>,
    active: Option<Res<ActiveLevel>>,
) {
    let Ok((root, mut node)) = roots.single_mut() else {
        return;
    };
    let Some(active) = active else { return };
    commands.entity(root).despawn_children();
    // Collapse out of flow outside the sandbox — campaign levels have no rename
    // rows, and the panel now sits in a flex Row next to the timetable, so an
    // empty-but-padded node would shove the timetable sideways.
    if !active.sandbox {
        node.display = Display::None;
        return;
    }
    node.display = Display::Flex;
    let font = ui_font.0.clone();
    commands.entity(root).with_children(|panel| {
        panel.spawn(text_bundle(&font, t("stations.title"), 15.0, TEXT_BRIGHT));
        let row = |panel: &mut ChildSpawnerCommands, prefix: String, label: &str, field: StationField| {
            panel
                .spawn(Node {
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    ..default()
                })
                .with_children(|r| {
                    r.spawn(text_bundle(&font, prefix, 13.0, TEXT_DIM));
                    text_field(r, &font, label, NAME_MAX, field);
                });
        };
        for source in &active.level.sources {
            row(
                panel,
                format!("Q{}", source.id.0),
                &source.label,
                StationField {
                    kind: StationKind::Source,
                    id: source.id.0,
                },
            );
        }
        for sink in &active.level.sinks {
            row(
                panel,
                format!("Z{}", sink.id.0),
                &sink.label,
                StationField {
                    kind: StationKind::Sink,
                    id: sink.id.0,
                },
            );
        }
    });
}

/// Applies committed name edits as one invertible `Rename*` op each.
fn station_field_commits(
    mut commits: MessageReader<TextFieldCommit>,
    fields: Query<&StationField>,
    active: Option<ResMut<ActiveLevel>>,
    mut editor: ResMut<Editor>,
) {
    let Some(mut active) = active else { return };
    if !active.sandbox {
        return;
    }
    for commit in commits.read() {
        let Ok(&StationField { kind, id }) = fields.get(commit.field) else {
            continue;
        };
        let after = commit.text.clone();
        // Bypass change detection (see module doc): keep the focused field alive.
        let level = &mut active.bypass_change_detection().level;
        let op = match kind {
            StationKind::Source => {
                level
                    .sources
                    .iter()
                    .find(|s| s.id.0 == id)
                    .map(|s| EditOp::RenameSource {
                        id: SourceId(id),
                        before: s.label.clone(),
                        after,
                    })
            }
            StationKind::Sink => level.sinks.iter().find(|s| s.id.0 == id).map(|s| {
                EditOp::RenameSink {
                    id: SinkId(id),
                    before: s.label.clone(),
                    after,
                }
            }),
        };
        if let Some(op) = op {
            do_op(&mut editor, level, op);
        }
    }
}
