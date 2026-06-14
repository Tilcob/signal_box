//! One-shot sound effects on the SFX channel, fired as observer events so any
//! system can `commands.trigger(SfxKind::…)` without knowing about audio.

use super::SfxChannel;
use super::assets::AudioAssets;
use bevy::prelude::*;
use bevy_kira_audio::{AudioChannel, AudioControl};

#[derive(Event, Clone, Copy)]
pub enum SfxKind {
    /// Any UI button press.
    ButtonClick,
    /// A switch placed on the board.
    Switch,
    /// A signal placed on the board.
    Signal,
    /// A train rolls into the world during a run.
    Rail,
}

pub(super) fn on_sfx(
    trigger: On<SfxKind>,
    audio: Option<Res<AudioAssets>>,
    channel: Res<AudioChannel<SfxChannel>>,
) {
    let Some(audio) = audio else {
        return;
    };
    let handle = match *trigger.event() {
        SfxKind::ButtonClick => &audio.button_click,
        SfxKind::Switch => &audio.switch_sound,
        SfxKind::Signal => &audio.signal_sound,
        SfxKind::Rail => &audio.rail_sound,
    };
    channel.play(handle.clone());
}

/// Global: any UI button press makes the click sound. One trigger per frame is
/// plenty even if several buttons change at once.
pub(super) fn button_click_sfx(
    buttons: Query<&Interaction, (Changed<Interaction>, With<Button>)>,
    mut commands: Commands,
) {
    if buttons.iter().any(|i| *i == Interaction::Pressed) {
        commands.trigger(SfxKind::ButtonClick);
    }
}
