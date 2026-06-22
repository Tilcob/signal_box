//! Reusable volume controls (music + SFX), shared by the pause menu and the
//! main menu. The buttons step the persisted master volume in `Progress`;
//! `crate::audio::apply_volume` picks the change up and applies it as decibels.

use bevy::prelude::*;
use bevy::text::Font;

use super::widgets::{TEXT_BRIGHT, TEXT_DIM, set_text, small_button, text_bundle};
use crate::i18n::t;
use crate::levels::Progress;

#[derive(Component, Clone, Copy)]
enum VolChannel {
    Music,
    Sfx,
}

/// A volume step button: which channel, and the direction (in 10% steps).
#[derive(Component, Clone, Copy)]
struct VolButton {
    channel: VolChannel,
    delta: i32,
}

/// The "NN%" readout for a channel, refreshed on every step.
#[derive(Component, Clone, Copy)]
struct VolLabel(VolChannel);

fn percent(fraction: f64) -> String {
    format!("{:.0}%", fraction * 100.0)
}

/// Spawns two rows (music, SFX), each `label  [-] NN% [+]`, under `parent`.
/// Reads the current `Progress` volumes for the initial readout.
pub(super) fn volume_controls(parent: &mut ChildSpawnerCommands, font: &Handle<Font>, progress: &Progress) {
    for (channel, frac, key) in [
        (VolChannel::Music, progress.music_volume, "options.music"),
        (VolChannel::Sfx, progress.sfx_volume, "options.sfx"),
    ] {
        parent
            .spawn(Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(8.0),
                ..default()
            })
            .with_children(|row| {
                row.spawn((
                    text_bundle(font, t(key), 15.0, TEXT_DIM),
                    Node {
                        width: Val::Px(80.0),
                        ..default()
                    },
                ));
                small_button(row, font, "-", VolButton { channel, delta: -1 });
                row.spawn((
                    text_bundle(font, percent(frac), 15.0, TEXT_BRIGHT),
                    Node {
                        width: Val::Px(44.0),
                        justify_content: JustifyContent::Center,
                        ..default()
                    },
                    VolLabel(channel),
                ));
                small_button(row, font, "+", VolButton { channel, delta: 1 });
            });
    }
}

/// Steps the master volume on a `+`/`-` press, persists it, and refreshes the
/// readouts. `Changed<Interaction>` keeps this near-free when no button moved;
/// the controls only exist inside the pause and main-menu overlays.
fn volume_clicks(
    interactions: Query<(&Interaction, &VolButton), Changed<Interaction>>,
    mut progress: ResMut<Progress>,
    mut labels: Query<(&VolLabel, &mut Text)>,
) {
    let mut dirty = false;
    for (interaction, btn) in &interactions {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let v = match btn.channel {
            VolChannel::Music => &mut progress.music_volume,
            VolChannel::Sfx => &mut progress.sfx_volume,
        };
        *v = (*v + f64::from(btn.delta) * 0.1).clamp(0.0, 1.0);
        dirty = true;
    }
    if !dirty {
        return;
    }
    progress.save();
    for (label, mut text) in &mut labels {
        let frac = match label.0 {
            VolChannel::Music => progress.music_volume,
            VolChannel::Sfx => progress.sfx_volume,
        };
        set_text(&mut text, percent(frac));
    }
}

pub(super) struct OptionsPlugin;

impl Plugin for OptionsPlugin {
    fn build(&self, app: &mut App) {
        // Ungated: the buttons live only in overlays that despawn with their
        // screen, and the `Changed<Interaction>` filter idles otherwise.
        app.add_systems(Update, volume_clicks);
    }
}
