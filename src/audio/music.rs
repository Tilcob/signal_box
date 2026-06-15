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
use std::time::Duration;
// Explicit imports (not the kira prelude glob) so `AudioSource` is
// unambiguously kira's, not Bevy's built-in one.
use bevy_kira_audio::{
    AudioChannel, AudioControl, AudioEasing, AudioInstance, AudioSource, AudioTween, PlaybackState,
};

/// Random silent pause between two level tracks, in seconds (uniform). The
/// example was "~25s with some spread" — tune the two ends to taste.
const GAP_SECS: std::ops::RangeInclusive<f32> = 20.0..=30.0;

/// Cross-fade length for every music start/stop. The desk's ASMR mix wants no
/// hard cuts (the reason this project picked kira). `const` works
/// because `AudioTween::new` is a const fn.
const FADE: AudioTween = AudioTween::new(Duration::from_millis(600), AudioEasing::Linear);

/// If a chosen level track has not actually started playing within this long,
/// its source is treated as failed/missing and skipped — otherwise a missing
/// file retries forever in kira and the playlist hangs silently.
const WATCHDOG_SECS: f32 = 5.0;

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
    /// A track is playing. `confirmed` flips true once kira reports the instance
    /// actually `Playing`, so a never-started track (still loading / missing
    /// source) is never mistaken for a finished one.
    Playing {
        handle: Handle<AudioInstance>,
        confirmed: bool,
        /// Counts up while the track has not yet started. If it elapses before
        /// `confirmed`, the source failed to load — skip instead of hanging.
        watchdog: Timer,
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
    // Clear the channel first: fades out whatever is playing AND cancels any
    // pending (still-loading or stuck) play command, so a slow track skipped by
    // the watchdog can't start on top of this one later.
    channel.stop().fade_out(FADE);
    let handle = channel
        .play(audio.level_tracks[i].clone())
        .fade_in(FADE)
        .handle();
    playlist.last = Some(i);
    playlist.phase = Phase::Playing {
        handle,
        confirmed: false,
        watchdog: Timer::from_seconds(WATCHDOG_SECS, TimerMode::Once),
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
    channel.stop().fade_out(FADE);
    channel.play(handle.clone()).looped().fade_in(FADE);
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
    // `play_track` stops the channel first, so the menu loop is faded out here.
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
    if audio.level_tracks.is_empty() {
        return; // nothing to play; also guards the `level_tracks[i]` indexing
    }
    match &mut playlist.phase {
        Phase::Idle => {}
        Phase::Playing {
            handle,
            confirmed,
            watchdog,
        } => match channel.state(handle) {
            PlaybackState::Playing { .. } => *confirmed = true,
            PlaybackState::Stopped if *confirmed => {
                playlist.phase = Phase::Gap(random_gap());
            }
            _ => {
                // Still queued/loading (or the brief startup blip). If it never
                // reaches `Playing`, the source is missing/failed — skip to the
                // next track instead of hanging the playlist forever.
                if !*confirmed && watchdog.tick(time.delta()).just_finished() {
                    warn!("level music: a track did not start within {WATCHDOG_SECS}s — skipping");
                    let i = playlist.pick(audio.level_tracks.len());
                    play_track(&channel, &audio, &mut playlist, i);
                }
            }
        },
        Phase::Gap(timer) => {
            if timer.tick(time.delta()).just_finished() {
                let i = playlist.pick(audio.level_tracks.len());
                play_track(&channel, &audio, &mut playlist, i);
            }
        }
    }
}
