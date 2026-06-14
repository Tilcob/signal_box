//! Background music on the dedicated music channel.
//!
//! Menu states share one calmly looping track (with a [`CurrentTrack`] guard so
//! re-entering a menu does not restart it). The desk (Edit + Run) instead runs a
//! random *playlist* over [`AudioAssets::level_tracks`]: one track at a time, no
//! looping, a random silent gap between tracks, and never the same track twice in
//! a row. The playlist keeps running across Edit<->Run toggles and only resets
//! when the player goes back to a menu.

use super::MusicChannel;
use super::assets::AudioAssets;
use bevy::prelude::*;
// Explicit imports (not the kira prelude glob) so `AudioSource` is
// unambiguously kira's, not Bevy's built-in one.
use bevy_kira_audio::{
    AudioChannel, AudioControl, AudioInstance, AudioSource, PlaybackState,
};

/// Random silent pause between two level tracks, in seconds (uniform). The
/// example was "~25s with some spread" — tune the two ends to taste.
const GAP_SECS: std::ops::RangeInclusive<f32> = 20.0..=30.0;

/// Which track is currently looping in the *menu* — avoids restarting it on
/// re-entry. The desk uses [`LevelPlaylist`] instead, not this enum.
#[derive(Resource, Default, PartialEq, Eq, Clone, Copy)]
pub(super) enum CurrentTrack {
    #[default]
    None,
    Menu,
    /// The desk playlist is in charge (set by `start_level_playlist`).
    Level,
}

/// State machine for the desk playlist.
#[derive(Resource, Default)]
pub(super) struct LevelPlaylist {
    /// Index of the previously started track, to avoid an immediate repeat.
    last: Option<usize>,
    phase: Phase,
}

#[derive(Default)]
enum Phase {
    /// Nothing scheduled — fresh from a menu. `start_level_playlist` kicks off.
    #[default]
    Idle,
    /// A track is playing. `confirmed` flips true once we've actually seen the
    /// instance in a live state: kira reports `Stopped` for a brand-new instance
    /// for a frame or two (command drained from the queue but not yet in the
    /// state map), so we must NOT treat that startup window as "finished".
    Playing {
        handle: Handle<AudioInstance>,
        confirmed: bool,
    },
    /// Silent pause before the next track.
    Gap(Timer),
}

impl LevelPlaylist {
    /// A random index `!= last`. Trivial for 0/1 tracks.
    fn pick(&self, n: usize) -> usize {
        if n <= 1 {
            return 0;
        }
        loop {
            let i = fastrand::usize(..n);
            if Some(i) != self.last {
                return i;
            }
        }
    }
}

fn random_gap() -> Timer {
    let g = fastrand::f32() * (GAP_SECS.end() - GAP_SECS.start()) + GAP_SECS.start();
    Timer::from_seconds(g, TimerMode::Once)
}

/// Start track index `i` on the music channel and record it as the playlist's
/// current `Playing` phase.
fn play_track(
    channel: &AudioChannel<MusicChannel>,
    audio: &AudioAssets,
    playlist: &mut LevelPlaylist,
    i: usize,
) {
    let handle = channel.play(audio.level_tracks[i].clone()).handle();
    playlist.last = Some(i);
    playlist.phase = Phase::Playing {
        handle,
        confirmed: false,
    };
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

/// Menu states (MainMenu, LevelSelect): looping calm track. Also resets the desk
/// playlist to `Idle` so the next desk entry starts a fresh random run.
pub(super) fn menu_music(
    channel: Res<AudioChannel<MusicChannel>>,
    audio: Option<Res<AudioAssets>>,
    mut current: ResMut<CurrentTrack>,
    mut playlist: ResMut<LevelPlaylist>,
) {
    let Some(audio) = audio else {
        return;
    };
    switch_to(&channel, &mut current, CurrentTrack::Menu, &audio.menu_music);
    playlist.phase = Phase::Idle;
}

/// Desk states (Edit, Run): start the playlist — but only if it isn't already
/// running, so toggling Edit<->Run does not restart the music.
pub(super) fn start_level_playlist(
    channel: Res<AudioChannel<MusicChannel>>,
    audio: Option<Res<AudioAssets>>,
    mut current: ResMut<CurrentTrack>,
    mut playlist: ResMut<LevelPlaylist>,
) {
    let Some(audio) = audio else {
        return;
    };
    if !matches!(playlist.phase, Phase::Idle) {
        return; // already playing (Edit<->Run toggle) — leave it be
    }
    if audio.level_tracks.is_empty() {
        return;
    }
    channel.stop(); // kill the menu loop before the first track
    *current = CurrentTrack::Level;
    let i = playlist.pick(audio.level_tracks.len());
    play_track(&channel, &audio, &mut playlist, i);
}

/// Drives the playlist while at the desk: detect track end, hold the gap, then
/// start the next random (non-repeating) track.
pub(super) fn drive_level_playlist(
    time: Res<Time>,
    channel: Res<AudioChannel<MusicChannel>>,
    audio: Option<Res<AudioAssets>>,
    mut playlist: ResMut<LevelPlaylist>,
) {
    let Some(audio) = audio else {
        return;
    };
    match &mut playlist.phase {
        Phase::Idle => {}
        Phase::Playing { handle, confirmed } => {
            match channel.state(handle) {
                PlaybackState::Stopped => {
                    // Only a real end if we'd previously seen it live; otherwise
                    // it's the startup window — wait for the instance to appear.
                    if *confirmed {
                        playlist.phase = Phase::Gap(random_gap());
                    }
                }
                _ => *confirmed = true,
            }
        }
        Phase::Gap(timer) => {
            if timer.tick(time.delta()).just_finished() {
                let i = playlist.pick(audio.level_tracks.len());
                play_track(&channel, &audio, &mut playlist, i);
            }
        }
    }
}
