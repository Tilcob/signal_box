//! Grid ↔ world coordinate mapping shared by board, editor and run.

use bevy::prelude::*;
use stellwerk_sim::grid::{Cell, Dir8, Point};

pub const CELL: f32 = 96.0;

pub fn point_world(p: Point) -> Vec2 {
    Vec2::new(p.x as f32, p.y as f32) * (CELL / 2.0)
}

pub fn cell_world(c: Cell) -> Vec2 {
    point_world(c.center_point())
}

pub fn connector_world(c: Cell, d: Dir8) -> Vec2 {
    point_world(c.connector_point(d))
}

/// Ready-to-draw geometry of a freight platform dock, so the board renderer
/// ([`super::draw`]) and the placement ghost ([`crate::editor`]) stay pixel-
/// identical (preview = result). The dock sits on ONE side of the track: a
/// platform-edge slab with a bright lip facing the rail, and a plain block
/// behind it (away from the rail).
pub struct Dock {
    /// Track axis (unit, toward the anchor connector).
    pub axis: Vec2,
    /// Platform-edge slab, a band along the track.
    pub slab_a: Vec2,
    pub slab_b: Vec2,
    pub slab_w: f32,
    /// Bright lip on the slab's rail-facing side.
    pub edge_a: Vec2,
    pub edge_b: Vec2,
    pub edge_w: f32,
    /// Plain block behind the slab: centre + full length/width (band for the
    /// board, `rect_2d` for the ghost).
    pub block_center: Vec2,
    pub block_len: f32,
    pub block_w: f32,
    /// Id-label anchor, kept clear of the dock (opposite side of the track).
    pub label: Vec2,
}

/// Build a [`Dock`] centred on `center` with the track running along `axis` (a
/// unit vector from the cell centre toward the anchor connector).
pub fn platform_dock(center: Vec2, axis: Vec2) -> Dock {
    // All offsets in board px (CELL = 96); the dock stays within the cell.
    // The slab (and its bright lip) span the FULL tile width; the block behind
    // stays short and centred.
    const SLAB_HALF: f32 = CELL / 2.0;
    const SLAB_OFF: f32 = 11.0;
    const SLAB_W: f32 = 8.0;
    const BLOCK_HALF: f32 = 10.0;
    const BLOCK_OFF: f32 = 21.0; // front touches the slab's back
    const BLOCK_W: f32 = 12.0;
    let perp = axis.perp();
    let slab_c = center + perp * SLAB_OFF;
    let edge_c = center + perp * (SLAB_OFF - SLAB_W / 2.0); // rail-facing surface
    Dock {
        axis,
        slab_a: slab_c - axis * SLAB_HALF,
        slab_b: slab_c + axis * SLAB_HALF,
        slab_w: SLAB_W,
        edge_a: edge_c - axis * SLAB_HALF,
        edge_b: edge_c + axis * SLAB_HALF,
        edge_w: 2.5,
        block_center: center + perp * BLOCK_OFF,
        block_len: BLOCK_HALF * 2.0,
        block_w: BLOCK_W,
        label: center - perp * 22.0,
    }
}

/// World position → cell under the cursor.
pub fn world_cell(pos: Vec2) -> Cell {
    Cell {
        x: (pos.x / CELL).floor() as i32,
        y: (pos.y / CELL).floor() as i32,
    }
}

/// Cells inside the bounding box of `buildable` that are NOT buildable — the
/// sandbox "blocked" holes, rendered as solid tiles. Derived from `buildable`
/// alone (no stored field bounds): an interior hole shows as a block, while
/// blocking a whole edge simply shrinks the box. Empty when `buildable` is.
pub fn blocked_cells(buildable: &[Cell]) -> Vec<Cell> {
    let (Some(min_x), Some(max_x), Some(min_y), Some(max_y)) = (
        buildable.iter().map(|c| c.x).min(),
        buildable.iter().map(|c| c.x).max(),
        buildable.iter().map(|c| c.y).min(),
        buildable.iter().map(|c| c.y).max(),
    ) else {
        return Vec::new();
    };
    let present: std::collections::BTreeSet<Cell> = buildable.iter().copied().collect();
    let mut out = Vec::new();
    for x in min_x..=max_x {
        for y in min_y..=max_y {
            let cell = Cell { x, y };
            if !present.contains(&cell) {
                out.push(cell);
            }
        }
    }
    out
}

/// Whether `cell` is a blocked hole: inside the buildable bounding box but not
/// itself buildable. Allocation-free, for the per-frame overlay hover check
/// (unlike [`blocked_cells`], which materializes the whole set for drawing).
pub fn is_blocked(buildable: &[Cell], cell: Cell) -> bool {
    let (Some(min_x), Some(max_x), Some(min_y), Some(max_y)) = (
        buildable.iter().map(|c| c.x).min(),
        buildable.iter().map(|c| c.x).max(),
        buildable.iter().map(|c| c.y).min(),
        buildable.iter().map(|c| c.y).max(),
    ) else {
        return false;
    };
    (min_x..=max_x).contains(&cell.x)
        && (min_y..=max_y).contains(&cell.y)
        && !buildable.contains(&cell)
}

/// Connector of `cell` nearest to a world position.
pub fn nearest_connector(cell: Cell, pos: Vec2) -> Dir8 {
    *Dir8::ALL
        .iter()
        .min_by(|a, b| {
            let da = connector_world(cell, **a).distance_squared(pos);
            let db = connector_world(cell, **b).distance_squared(pos);
            da.total_cmp(&db)
        })
        .expect("ALL is non-empty")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect(w: i32, h: i32) -> Vec<Cell> {
        (0..w).flat_map(|x| (0..h).map(move |y| Cell { x, y })).collect()
    }

    #[test]
    fn interior_hole_is_a_block() {
        let mut cells = rect(3, 3);
        cells.retain(|c| *c != Cell { x: 1, y: 1 });
        assert_eq!(blocked_cells(&cells), vec![Cell { x: 1, y: 1 }]);
    }

    #[test]
    fn removed_edge_shrinks_the_box_instead_of_blocking() {
        // Drop the whole top row: the bbox shrinks, so nothing is "blocked".
        let cells: Vec<Cell> = rect(3, 3).into_iter().filter(|c| c.y < 2).collect();
        assert!(blocked_cells(&cells).is_empty());
    }

    #[test]
    fn empty_buildable_has_no_blocks() {
        assert!(blocked_cells(&[]).is_empty());
    }
}
