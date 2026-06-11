//! Y-sorting for top-down depth: entities lower on screen render in front.

use bevy::prelude::*;

/// Z band for Y-sorted entities: `z = BIAS - feet_y * SCALE`. Sits above the
/// LDtk tilemap layers (z ≈ 1–3) and below overlays/UI and the camera (999).
const YSORT_BIAS: f32 = 50.0;
const YSORT_SCALE: f32 = 0.05;

/// Depth-sorts the entity by its feet position. `offset_y` shifts the sort
/// point from the sprite origin down to the feet (use the collider offset).
#[derive(Component, Default)]
pub struct YSort {
    pub offset_y: f32,
}

impl YSort {
    /// Template API — for sprites whose feet sit below the origin.
    #[allow(dead_code)]
    pub fn new(offset_y: f32) -> Self {
        Self { offset_y }
    }
}

pub struct YSortPlugin;

impl Plugin for YSortPlugin {
    fn build(&self, app: &mut App) {
        // PostUpdate, before transform propagation: the new Z is final for this
        // frame and gameplay systems in Update never see a half-written Z.
        app.add_systems(
            PostUpdate,
            apply_y_sort.before(TransformSystems::Propagate),
        );
    }
}

fn apply_y_sort(mut query: Query<(&YSort, &mut Transform)>) {
    for (ysort, mut transform) in &mut query {
        transform.translation.z = YSORT_BIAS - (transform.translation.y + ysort.offset_y) * YSORT_SCALE;
    }
}
