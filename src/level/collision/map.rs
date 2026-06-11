//! Grid storage + circle collision queries (`is_circle_clear`, `sweep_circle`),
//! plus the builder that derives the grid from rendered solid-cell positions.

use bevy::prelude::*;

/// How many rings (in tiles) [`CollisionMap::find_nearest_clear_position`]
/// searches around a blocked position before giving up.
const NEAREST_CLEAR_MAX_RING: i32 = 8;

/// Walkability grid in world space. Row-major `Vec<bool>` (`true` = solid),
/// origin at the bottom-left corner of the grid.
#[derive(Resource)]
pub struct CollisionMap {
    solid: Vec<bool>,
    width: i32,
    height: i32,
    tile_size: f32,
    origin: Vec2,
}

impl CollisionMap {
    pub fn new(width: i32, height: i32, tile_size: f32, origin: Vec2) -> Self {
        Self {
            solid: vec![false; (width * height).max(0) as usize],
            width,
            height,
            tile_size,
            origin,
        }
    }

    #[inline]
    fn idx(&self, x: i32, y: i32) -> usize {
        (y * self.width + x) as usize
    }

    #[inline]
    pub fn in_bounds(&self, x: i32, y: i32) -> bool {
        x >= 0 && x < self.width && y >= 0 && y < self.height
    }

    pub fn set_solid(&mut self, x: i32, y: i32, solid: bool) {
        if self.in_bounds(x, y) {
            let idx = self.idx(x, y);
            self.solid[idx] = solid;
        }
    }

    /// Out-of-bounds counts as solid, so the level border always blocks.
    pub fn is_solid(&self, x: i32, y: i32) -> bool {
        if self.in_bounds(x, y) {
            self.solid[self.idx(x, y)]
        } else {
            true
        }
    }

    pub fn world_to_grid(&self, world: Vec2) -> IVec2 {
        ((world - self.origin) / self.tile_size).floor().as_ivec2()
    }

    /// World position of the tile centre.
    pub fn grid_to_world(&self, x: i32, y: i32) -> Vec2 {
        self.origin + (Vec2::new(x as f32, y as f32) + 0.5) * self.tile_size
    }

    /// True when a circle at `center` overlaps no solid tile and stays inside
    /// the grid bounds.
    pub fn is_circle_clear(&self, center: Vec2, radius: f32) -> bool {
        if radius <= 0.0 {
            let g = self.world_to_grid(center);
            return !self.is_solid(g.x, g.y);
        }

        let min = self.world_to_grid(center - Vec2::splat(radius));
        let max = self.world_to_grid(center + Vec2::splat(radius));

        for gy in min.y..=max.y {
            for gx in min.x..=max.x {
                if self.is_solid(gx, gy) && self.circle_intersects_tile(center, radius, gx, gy) {
                    return false;
                }
            }
        }
        true
    }

    fn circle_intersects_tile(&self, center: Vec2, radius: f32, gx: i32, gy: i32) -> bool {
        let tile_min = self.origin + Vec2::new(gx as f32, gy as f32) * self.tile_size;
        let tile_max = tile_min + Vec2::splat(self.tile_size);
        let closest = center.clamp(tile_min, tile_max);
        // Strictly `<`: merely *touching* a wall is not a collision, otherwise
        // an actor flush against a wall could never stand there.
        center.distance_squared(closest) < radius * radius
    }

    /// Moves a circle from `start` towards `end`, sliding along walls on the
    /// blocked axis. Returns the furthest reachable centre position.
    pub fn sweep_circle(&self, start: Vec2, end: Vec2, radius: f32) -> Vec2 {
        let delta = end - start;
        if delta.length_squared() < 1e-6 {
            return start;
        }

        // Sub-steps of a quarter tile so fast movement can't tunnel through.
        let max_step = self.tile_size * 0.25;
        let steps = (delta.length() / max_step).ceil().max(1.0) as i32;
        let step = delta / steps as f32;

        let mut pos = start;
        for _ in 0..steps {
            let candidate = pos + step;
            if self.is_circle_clear(candidate, radius) {
                pos = candidate;
                continue;
            }
            // Slide: try each axis on its own.
            let try_x = Vec2::new(candidate.x, pos.y);
            if self.is_circle_clear(try_x, radius) {
                pos = try_x;
                continue;
            }
            let try_y = Vec2::new(pos.x, candidate.y);
            if self.is_circle_clear(try_y, radius) {
                pos = try_y;
                continue;
            }
            break; // Fully blocked.
        }
        pos
    }

    /// True when a circle of `radius` can travel the whole segment without
    /// clipping a wall — useful for line-of-movement checks and AI shortcuts.
    /// Template API — unused until game code needs it.
    #[allow(dead_code)]
    pub fn is_circle_path_clear(&self, from: Vec2, to: Vec2, radius: f32) -> bool {
        let dist = from.distance(to);
        if dist < f32::EPSILON {
            return self.is_circle_clear(from, radius);
        }
        let steps = (dist / (self.tile_size * 0.25)).ceil().max(1.0) as i32;
        (0..=steps).all(|i| self.is_circle_clear(from.lerp(to, i as f32 / steps as f32), radius))
    }

    /// Nearest position (searching outward ring by ring, tile centres) where a
    /// circle of `radius` fits. Safety net against spawning inside a wall.
    pub fn find_nearest_clear_position(&self, from: Vec2, radius: f32) -> Option<Vec2> {
        if self.is_circle_clear(from, radius) {
            return Some(from);
        }
        let start = self.world_to_grid(from);
        for ring in 1..=NEAREST_CLEAR_MAX_RING {
            let mut best: Option<(f32, Vec2)> = None;
            for dy in -ring..=ring {
                for dx in -ring..=ring {
                    if dx.abs().max(dy.abs()) != ring {
                        continue; // Only the ring's border cells.
                    }
                    let pos = self.grid_to_world(start.x + dx, start.y + dy);
                    if self.is_circle_clear(pos, radius) {
                        let d = pos.distance_squared(from);
                        if best.is_none_or(|(bd, _)| d < bd) {
                            best = Some((d, pos));
                        }
                    }
                }
            }
            if let Some((_, pos)) = best {
                return Some(pos);
            }
        }
        None
    }

    /// World centres of all solid tiles — used by the dev collision gizmos.
    #[cfg_attr(not(feature = "dev"), allow(dead_code))]
    pub fn solid_world_centers(&self) -> impl Iterator<Item = Vec2> + '_ {
        (0..self.height).flat_map(move |y| {
            (0..self.width)
                .filter(move |&x| self.solid[self.idx(x, y)])
                .map(move |x| self.grid_to_world(x, y))
        })
    }

    pub fn width(&self) -> i32 {
        self.width
    }

    pub fn height(&self) -> i32 {
        self.height
    }

    pub fn tile_size(&self) -> f32 {
        self.tile_size
    }
}

/// Builds a [`CollisionMap`] from the rendered world-space centres of solid
/// cells (their `GlobalTransform`s). Because cells sit on a regular grid,
/// integer indices fall out of rounding relative to the minimum centre.
///
/// Callers guarantee `centers` is non-empty and `tile_size > 0`.
pub fn build_from_centers(centers: &[Vec2], tile_size: f32) -> CollisionMap {
    let mut min = Vec2::splat(f32::MAX);
    let mut max = Vec2::splat(f32::MIN);
    for c in centers {
        min = min.min(*c);
        max = max.max(*c);
    }

    let width = ((max.x - min.x) / tile_size).round() as i32 + 1;
    let height = ((max.y - min.y) / tile_size).round() as i32 + 1;
    let origin = min - Vec2::splat(tile_size * 0.5);

    let mut map = CollisionMap::new(width, height, tile_size, origin);
    for c in centers {
        let gx = ((c.x - min.x) / tile_size).round() as i32;
        let gy = ((c.y - min.y) / tile_size).round() as i32;
        map.set_solid(gx, gy, true);
    }
    map
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 10×10 fully-walkable map, tile size 16, origin at (0,0).
    fn open_map() -> CollisionMap {
        CollisionMap::new(10, 10, 16.0, Vec2::ZERO)
    }

    #[test]
    fn world_grid_roundtrip_hits_tile_centre() {
        let map = open_map();
        for (gx, gy) in [(0, 0), (3, 7), (9, 9)] {
            let world = map.grid_to_world(gx, gy);
            assert_eq!(map.world_to_grid(world), IVec2::new(gx, gy));
        }
    }

    #[test]
    fn out_of_bounds_is_solid() {
        let map = open_map();
        assert!(map.is_solid(-1, 0));
        assert!(map.is_solid(0, 10));
        assert!(!map.is_solid(0, 0));
    }

    #[test]
    fn is_circle_clear_respects_radius_near_wall() {
        let mut map = open_map();
        map.set_solid(5, 5, true);
        // Centre of tile (4,5) is 8px from the wall tile's edge.
        let center = map.grid_to_world(4, 5);
        assert!(map.is_circle_clear(center, 4.0), "radius 4 fits");
        assert!(!map.is_circle_clear(center, 9.0), "radius 9 clips the wall");
    }

    #[test]
    fn sweep_circle_slides_along_wall() {
        let mut map = open_map();
        for x in 0..10 {
            map.set_solid(x, 6, true); // Horizontal wall above.
        }
        let start = map.grid_to_world(5, 5);
        // Try to move up-right into the wall: Y blocks, X slides.
        let end = map.sweep_circle(start, start + Vec2::new(16.0, 16.0), 4.0);
        assert!(end.x > start.x, "slid along X");
        assert!(end.y < map.grid_to_world(0, 6).y, "did not enter the wall");
    }

    #[test]
    fn build_from_centers_places_solids_on_the_grid() {
        // Three wall cells of a 16px grid, offset into negative world space
        // (LDtk levels render at y <= 0).
        let centers = [
            Vec2::new(8.0, -8.0),
            Vec2::new(24.0, -8.0),
            Vec2::new(8.0, -40.0),
        ];
        let map = build_from_centers(&centers, 16.0);
        assert_eq!((map.width(), map.height()), (2, 3));
        for c in centers {
            let g = map.world_to_grid(c);
            assert!(map.is_solid(g.x, g.y), "cell at {c:?} is solid");
            assert_eq!(map.grid_to_world(g.x, g.y), c, "grid centre matches");
        }
        // The gap between the two columns stays walkable.
        let gap = map.world_to_grid(Vec2::new(24.0, -40.0));
        assert!(!map.is_solid(gap.x, gap.y));
    }

    #[test]
    fn nearest_clear_position_escapes_a_wall() {
        let mut map = open_map();
        map.set_solid(5, 5, true);
        let inside = map.grid_to_world(5, 5);
        let clear = map
            .find_nearest_clear_position(inside, 4.0)
            .expect("a neighbouring tile is clear");
        assert!(map.is_circle_clear(clear, 4.0));
    }
}
