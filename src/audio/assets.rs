//! Audio handles, loaded once at startup. The `Option<Res<AudioAssets>>` guards
//! elsewhere keep the game silent-but-fine until this resource exists (and if a
//! file is missing, kira just plays nothing — no crash).

use bevy::prelude::*;
use bevy_kira_audio::AudioSource;

/// Level-Musik-Pool: am Pult (Edit + Run) spielt eine zufällige Playlist daraus,
/// je ein Track, danach eine Stille-Pause, dann der nächste (nie zweimal
/// derselbe direkt hintereinander — siehe `music::drive_level_playlist`).
/// `AssetServer` kann zur Laufzeit keinen Ordner auflisten (wasm-tauglich muss
/// es explizit sein), darum die Dateinamen hart hier.
const LEVEL_TRACKS: &[&str] = &[
    "audio/music/4379051-about-trains-passing-by-179886.ogg",
    "audio/music/4379051-siberia-express-192132.ogg",
    "audio/music/bransboynd-night-city-418052.ogg",
    "audio/music/juliush-metro-urban-adventure-music-288519.ogg",
    "audio/music/songshu888-nighttrain-145794.ogg",
];

#[derive(Resource)]
pub struct AudioAssets {
    pub menu_music: Handle<AudioSource>,
    /// Pool für die Pult-Playlist (Reihenfolge = Index, siehe `LevelPlaylist`).
    pub level_tracks: Vec<Handle<AudioSource>>,
    pub button_click: Handle<AudioSource>,
    pub switch_sound: Handle<AudioSource>,
    pub rail_sound: Handle<AudioSource>,
    pub signal_sound: Handle<AudioSource>,
    pub train_horn_sound: Handle<AudioSource>,
}

/// Builds the audio handles. Called at plugin-BUILD time (not via a `Startup`
/// system), because `bevy_state` runs the initial `OnEnter(MainMenu)` transition
/// before `PreStartup` — a startup system would insert `AudioAssets` too late and
/// `menu_music` (a one-shot `OnEnter`) would find nothing and leave the title
/// screen silent forever. Same race + fix as `FontPlugin` (see `src/font.rs`).
pub(super) fn build_audio_assets(asset_server: &AssetServer) -> AudioAssets {
    AudioAssets {
        menu_music: asset_server.load("audio/music/grand_project-technology-modern-electronic-railway-track-470218.ogg"),
        level_tracks: LEVEL_TRACKS
            .iter()
            .map(|path| asset_server.load(*path))
            .collect(),
        button_click: asset_server.load("audio/sfx/button_click.wav"),
        switch_sound: asset_server.load("audio/sfx/railway-switch-track.wav"),
        rail_sound: asset_server.load("audio/sfx/rail.wav"),
        signal_sound: asset_server.load("audio/sfx/signal_click.wav"),
        train_horn_sound: asset_server.load("audio/sfx/train-horn.wav"),
    }
}
