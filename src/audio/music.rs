//! Background music on the dedicated music channel: looped, one track at a
//! time, with a [`CurrentTrack`] guard so re-entering a state with the same
//! track does not restart it.

use super::MusicChannel;
use super::assets::AudioAssets;
use bevy::prelude::*;
// Explicit imports (not the kira prelude glob) so `AudioSource` is
// unambiguously kira's, not Bevy's built-in one.
use bevy_kira_audio::{AudioChannel, AudioControl, AudioSource};

/// Which track is currently looping — avoids restarting it on re-entry.
#[derive(Resource, Default, PartialEq, Eq, Clone, Copy)]
pub(super) enum CurrentTrack {
    #[default]
    None,
    Menu,
    Level,
}

fn switch_to(
    channel: &AudioChannel<MusicChannel>,
    current: &mut CurrentTrack,
    want: CurrentTrack,
    handle: &Handle<AudioSource>,
) {
    if *current == want {
        return;
    }
    channel.stop();
    channel.play(handle.clone()).looped();
    *current = want;
}

pub(super) fn menu_music(
    channel: Res<AudioChannel<MusicChannel>>,
    audio: Option<Res<AudioAssets>>,
    mut current: ResMut<CurrentTrack>,
) {
    let Some(audio) = audio else {
        return;
    };
    switch_to(&channel, &mut current, CurrentTrack::Menu, &audio.menu_music);
}

pub(super) fn level_music(
    channel: Res<AudioChannel<MusicChannel>>,
    audio: Option<Res<AudioAssets>>,
    mut current: ResMut<CurrentTrack>,
) {
    let Some(audio) = audio else {
        return;
    };
    switch_to(
        &channel,
        &mut current,
        CurrentTrack::Level,
        &audio.gameplay_music,
    );
}
