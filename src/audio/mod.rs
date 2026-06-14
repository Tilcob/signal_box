//! Audio (GDD §11): `bevy_kira_audio` for fades, loops and many concurrent
//! one-shots — the control the ASMR "Pult" sound design needs (GDD §12 chose
//! kira over Bevy's built-in audio for exactly this). Two channels give music
//! and SFX independent volume. Bevy's own audio plugin is disabled in `main`
//! so the two backends don't fight over the output device.

mod assets;
mod music;
mod sfx;

pub use sfx::SfxKind;

use bevy::prelude::*;
use bevy_kira_audio::{AudioApp, AudioPlugin};

use crate::state::GameState;

/// Music channel marker (looping background tracks, one at a time).
#[derive(Resource)]
pub struct MusicChannel;
/// SFX channel marker (short one-shots, may overlap).
#[derive(Resource)]
pub struct SfxChannel;

pub struct AudioManagerPlugin;

impl Plugin for AudioManagerPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(AudioPlugin)
            .add_audio_channel::<MusicChannel>()
            .add_audio_channel::<SfxChannel>()
            .init_resource::<music::CurrentTrack>()
            .add_systems(Startup, assets::load_audio_assets)
            .add_observer(sfx::on_sfx)
            .add_systems(Update, sfx::button_click_sfx)
            // Menu-side states share the calm menu track; the desk (Edit/Run)
            // gets the gameplay track. CurrentTrack dedups re-entries.
            .add_systems(OnEnter(GameState::MainMenu), music::menu_music)
            .add_systems(OnEnter(GameState::LevelSelect), music::menu_music)
            .add_systems(OnEnter(GameState::Edit), music::level_music)
            .add_systems(OnEnter(GameState::Run), music::level_music);
    }
}
