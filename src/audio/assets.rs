//! Audio handles, loaded once at startup. The `Option<Res<AudioAssets>>` guards
//! elsewhere keep the game silent-but-fine until this resource exists (and if a
//! file is missing, kira just plays nothing — no crash).

use bevy::prelude::*;
use bevy_kira_audio::AudioSource;

#[derive(Resource)]
pub struct AudioAssets {
    pub menu_music: Handle<AudioSource>,
    pub gameplay_music: Handle<AudioSource>,
    pub button_click: Handle<AudioSource>,
    pub switch_sound: Handle<AudioSource>,
    pub rail_sound: Handle<AudioSource>,
    pub signal_sound: Handle<AudioSource>,
}

pub(super) fn load_audio_assets(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.insert_resource(AudioAssets {
        menu_music: asset_server.load("audio/music/menu_music.ogg"),
        gameplay_music: asset_server.load("audio/music/gameplay_music.ogg"),
        button_click: asset_server.load("audio/sfx/button_click.wav"),
        switch_sound: asset_server.load("audio/sfx/switch.wav"),
        rail_sound: asset_server.load("audio/sfx/rail.wav"),
        signal_sound: asset_server.load("audio/sfx/signal.wav"),
    });
}
