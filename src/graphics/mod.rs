//! Presentation: follow camera, top-down depth sorting, sprite animation.

pub mod animation;
pub mod camera;
pub mod y_sort;

use bevy::prelude::*;

pub struct GraphicsPlugin;

impl Plugin for GraphicsPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            camera::CameraPlugin,
            y_sort::YSortPlugin,
            animation::AnimationPlugin,
        ));
    }
}
