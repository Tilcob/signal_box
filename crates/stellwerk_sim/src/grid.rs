//! The square grid: cells, their eight connectors, and the shared connector
//! point lattice.
//!
//! Coordinates are doubled for points: a cell `(x, y)` spans
//! `[2x, 2x+2] × [2y, 2y+2]`, its center sits at `(2x+1, 2y+1)`, edge
//! midpoints and corners at the even/odd mixtures. Two neighboring cells
//! therefore agree on the *same* `Point` for their shared connector — no
//! pairing logic needed.

use crate::units::Len;
use crate::units::segment_lengths::{HALF_CARDINAL, HALF_DIAGONAL};
use serde::{Deserialize, Serialize};

/// A cell coordinate on the build grid.
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Cell {
    pub x: i32,
    pub y: i32,
}

impl Cell {
    pub fn neighbor(self, dir: Dir8) -> Cell {
        let (dx, dy) = dir.cell_offset();
        Cell {
            x: self.x + dx,
            y: self.y + dy,
        }
    }

    /// Center of the cell on the point lattice.
    pub fn center_point(self) -> Point {
        Point {
            x: i64::from(self.x) * 2 + 1,
            y: i64::from(self.y) * 2 + 1,
        }
    }

    /// The connector (edge midpoint or corner) on the point lattice. Shared
    /// with the matching connector of the neighbor in that direction.
    pub fn connector_point(self, dir: Dir8) -> Point {
        let (dx, dy) = dir.cell_offset();
        let center = self.center_point();
        Point {
            x: center.x + i64::from(dx),
            y: center.y + i64::from(dy),
        }
    }
}

/// The eight connectors of a cell: four edge midpoints (cardinal) and four
/// corners (diagonal). The index order is clockwise from north — the basis for
/// [`Dir8::angular_distance`].
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Dir8 {
    N,
    NE,
    E,
    SE,
    S,
    SW,
    W,
    NW,
}

impl Dir8 {
    pub const ALL: [Dir8; 8] = [
        Dir8::N,
        Dir8::NE,
        Dir8::E,
        Dir8::SE,
        Dir8::S,
        Dir8::SW,
        Dir8::W,
        Dir8::NW,
    ];

    pub fn index(self) -> u8 {
        self as u8
    }

    pub fn opposite(self) -> Dir8 {
        Self::ALL[((self.index() + 4) % 8) as usize]
    }

    /// Rotate by `steps` × 45° (clockwise for positive, counter-clockwise for
    /// negative). The shared turn operator behind the editor's R/T keys and
    /// the radial track menu; rotation preserves [`pair_len`] legality.
    pub fn rotate(self, steps: i32) -> Dir8 {
        Self::ALL[(i32::from(self.index()) + steps).rem_euclid(8) as usize]
    }

    pub fn is_cardinal(self) -> bool {
        self.index().is_multiple_of(2)
    }

    /// Offset of the neighbor cell reached by crossing this connector.
    pub fn cell_offset(self) -> (i32, i32) {
        match self {
            Dir8::N => (0, 1),
            Dir8::NE => (1, 1),
            Dir8::E => (1, 0),
            Dir8::SE => (1, -1),
            Dir8::S => (0, -1),
            Dir8::SW => (-1, -1),
            Dir8::W => (-1, 0),
            Dir8::NW => (-1, 1),
        }
    }

    /// Length of the stub from this connector to the cell center.
    pub fn half_len(self) -> Len {
        if self.is_cardinal() {
            HALF_CARDINAL
        } else {
            HALF_DIAGONAL
        }
    }

    /// Angular steps (0..=4) between two connectors; 1 step = 45°.
    pub fn angular_distance(self, other: Dir8) -> u8 {
        let d = self.index().abs_diff(other.index());
        d.min(8 - d)
    }
}

/// A point on the doubled-coordinate lattice (see module docs). Cell centers
/// have two odd coordinates; connectors at least one even coordinate — the
/// two kinds can never collide.
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Point {
    pub x: i64,
    pub y: i64,
}

/// Track legality of a connector pair within one cell, with its length.
///
/// Legal if the turn is at most 90°: angular distance 4 (straight/diagonal),
/// 3 (45° turn) or 2 (90° turn). Distance 0–1 is a kink and `None`.
pub fn pair_len(a: Dir8, b: Dir8) -> Option<Len> {
    if a.angular_distance(b) >= 2 {
        Some(a.half_len() + b.half_len())
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::units::segment_lengths::*;

    #[test]
    fn neighbors_share_connector_points() {
        let c = Cell { x: 0, y: 0 };
        // Cardinal: N of (0,0) == S of (0,1).
        assert_eq!(
            c.connector_point(Dir8::N),
            c.neighbor(Dir8::N).connector_point(Dir8::S)
        );
        // Diagonal: NE of (0,0) == SW of (1,1).
        assert_eq!(
            c.connector_point(Dir8::NE),
            c.neighbor(Dir8::NE).connector_point(Dir8::SW)
        );
    }

    #[test]
    fn opposites() {
        assert_eq!(Dir8::N.opposite(), Dir8::S);
        assert_eq!(Dir8::NE.opposite(), Dir8::SW);
        assert_eq!(Dir8::W.opposite(), Dir8::E);
    }

    #[test]
    fn rotation_wraps_both_ways_and_preserves_legality() {
        assert_eq!(Dir8::N.rotate(2), Dir8::E);
        assert_eq!(Dir8::N.rotate(-1), Dir8::NW);
        assert_eq!(Dir8::N.rotate(8), Dir8::N);
        assert_eq!(Dir8::N.rotate(-8), Dir8::N);
        assert_eq!(Dir8::N.rotate(4), Dir8::N.opposite());
        // Rotating both connectors of a legal pair keeps it legal.
        for a in Dir8::ALL {
            for b in Dir8::ALL {
                if pair_len(a, b).is_some() {
                    for k in -3..=3 {
                        assert!(pair_len(a.rotate(k), b.rotate(k)).is_some());
                    }
                }
            }
        }
    }

    #[test]
    fn pair_lengths_match_frozen_table() {
        assert_eq!(pair_len(Dir8::W, Dir8::E), Some(STRAIGHT));
        assert_eq!(pair_len(Dir8::SW, Dir8::NE), Some(DIAGONAL));
        assert_eq!(pair_len(Dir8::W, Dir8::NE), Some(CURVE_45));
        assert_eq!(pair_len(Dir8::N, Dir8::E), Some(CURVE_90));
        // Corner-to-corner 90° turn composes of two diagonal halves.
        assert_eq!(pair_len(Dir8::NE, Dir8::SE), Some(DIAGONAL));
    }

    #[test]
    fn kinks_are_illegal() {
        assert_eq!(pair_len(Dir8::N, Dir8::N), None);
        assert_eq!(pair_len(Dir8::N, Dir8::NE), None);
        assert_eq!(pair_len(Dir8::W, Dir8::NW), None);
    }
}
