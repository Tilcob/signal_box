//! Generic sprite-sheet animation for any entity with an atlas `Sprite`.
//!
//! ```ignore
//! commands.spawn((
//!     Sprite::from_atlas_image(texture, TextureAtlas { layout, index: 0 }),
//!     SpriteAnimation::looping(0, 3, 8.0), // frames 0..=3 at 8 FPS
//! ));
//! ```
//! Switching clips (idle/walk/…) is just inserting a new `SpriteAnimation`.

use bevy::prelude::*;

use crate::core::GameplaySet;

/// Plays a contiguous range of atlas frames `[first, last]` in a loop.
#[derive(Component, Clone)]
pub struct SpriteAnimation {
    pub first: usize,
    pub last: usize,
    timer: Timer,
}

// Template API — unused until game code spawns its first animated sprite.
#[allow(dead_code)]
impl SpriteAnimation {
    pub fn looping(first: usize, last: usize, fps: f32) -> Self {
        Self {
            first,
            last: last.max(first),
            timer: Timer::from_seconds(1.0 / fps.max(0.01), TimerMode::Repeating),
        }
    }

    pub fn set_fps(&mut self, fps: f32) {
        self.timer
            .set_duration(std::time::Duration::from_secs_f32(1.0 / fps.max(0.01)));
    }
}

pub struct AnimationPlugin;

impl Plugin for AnimationPlugin {
    fn build(&self, app: &mut App) {
        // In `GameplaySet`, so animations freeze while paused.
        app.add_systems(Update, animate_sprites.in_set(GameplaySet));
    }
}

fn animate_sprites(time: Res<Time>, mut query: Query<(&mut SpriteAnimation, &mut Sprite)>) {
    for (mut animation, mut sprite) in &mut query {
        animation.timer.tick(time.delta());
        if !animation.timer.just_finished() {
            continue;
        }
        let Some(atlas) = sprite.texture_atlas.as_mut() else {
            continue;
        };
        // Also resets indices left behind by a previous, longer clip.
        atlas.index = if atlas.index < animation.first || atlas.index >= animation.last {
            animation.first
        } else {
            atlas.index + 1
        };
    }
}
