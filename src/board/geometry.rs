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
