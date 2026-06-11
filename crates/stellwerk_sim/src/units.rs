//! Deterministic base units of the simulation core.
//!
//! Everything in `stellwerk_sim` measures with these newtypes — never raw
//! numbers, never floats. See `lib.rs` for the full determinism contract.

use serde::{Deserialize, Serialize};
use std::ops::{Add, Mul, Sub};

/// Fixed segment lengths.
///
/// Geometry model (frozen in M0): every track piece runs as a polyline
/// through its cell center — one "stub" from each used connector to the
/// center. All lengths therefore compose of the two half-lengths below.
///
/// These values are **frozen after M0** and are part of the deterministic
/// simulation. They must never change, otherwise:
///   - Replay hashes become invalid,
///   - Old replays produce different outcomes,
///   - Personal bests and shared solution codes stop being comparable.
pub mod segment_lengths {
    use crate::units::Len;

    /// Cell center to an edge midpoint (half a cardinal crossing).
    pub const HALF_CARDINAL: Len = Len(500);
    /// Cell center to a corner (half a diagonal crossing; 500·√2, rounded).
    pub const HALF_DIAGONAL: Len = Len(707);

    /// Straight across the cell (W–E, N–S).
    pub const STRAIGHT: Len = Len(HALF_CARDINAL.0 * 2);
    /// Diagonal across the cell (SW–NE, NW–SE).
    pub const DIAGONAL: Len = Len(HALF_DIAGONAL.0 * 2);
    /// 45° turn (cardinal ↔ non-adjacent corner, e.g. W–NE).
    pub const CURVE_45: Len = Len(HALF_CARDINAL.0 + HALF_DIAGONAL.0);
    /// 90° turn (cardinal ↔ cardinal, e.g. N–E; modeled via the center,
    /// not as an arc — consistency beats geometric realism).
    pub const CURVE_90: Len = Len(HALF_CARDINAL.0 * 2);

    /// Anti-tunneling bound: no train speed may reach the shortest possible
    /// edge (a cardinal stub). Enforced by layout validation.
    pub const MAX_SPEED_EXCLUSIVE: i64 = HALF_CARDINAL.0;
}

/// Stall fallback (plan §4.4): this many consecutive ticks without any
/// movement, spawn or arrival — while the schedule is unfinished — end the
/// run as `Outcome::Stalled`. Frozen like the length table.
pub const STALL_TICKS: u64 = 600;

/// Length in integer length units (LE). 1 cell edge = 1000 LE.
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Len(pub i64);

impl Len {
    pub fn get_length(self) -> i64 {
        self.0
    }
}

impl Add for Len {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self(self.0 + rhs.0)
    }
}

impl Sub for Len {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        Self(self.0 - rhs.0)
    }
}

/// Absolute simulation time. Nominal rate: 10 ticks/s (frontend concern only).
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Tick(pub u64);

impl Tick {
    pub fn get_ticks(self) -> u64 {
        self.0
    }
}

impl Add<TickDelta> for Tick {
    type Output = Tick;

    fn add(self, rhs: TickDelta) -> Self::Output {
        Self(self.0 + rhs.0)
    }
}

impl Sub<Tick> for Tick {
    type Output = TickDelta;

    /// Panics on underflow (`overflow-checks = true` is set for all profiles
    /// in the workspace root — a deterministic panic beats a silent wrap).
    fn sub(self, rhs: Self) -> Self::Output {
        TickDelta(self.0 - rhs.0)
    }
}

/// A span of simulation time.
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct TickDelta(pub u64);

impl TickDelta {
    pub fn get_ticks(self) -> u64 {
        self.0
    }
}

/// Speed in LE per tick.
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Speed(pub i64);

impl Speed {
    pub fn get_speed(self) -> i64 {
        self.0
    }
}

impl Mul<TickDelta> for Speed {
    type Output = Len;

    fn mul(self, rhs: TickDelta) -> Self::Output {
        Len(self.0 * rhs.0 as i64)
    }
}

macro_rules! id_type {
    ($(#[$doc:meta])* $name:ident) => {
        $(#[$doc])*
        #[derive(
            Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash,
            Serialize, Deserialize,
        )]
        pub struct $name(pub u32);

        impl $name {
            pub fn get_id(self) -> u32 {
                self.0
            }
        }
    };
}

id_type!(TrainId);
id_type!(NodeId);
id_type!(EdgeId);
id_type!(BlockId);
id_type!(SignalId);
id_type!(SwitchId);
id_type!(
    /// A source defined by the level (trains enter the world here).
    SourceId
);
id_type!(
    /// A sink defined by the level (trains leave the world here).
    SinkId
);
id_type!(
    /// Train class (e.g. freight, commuter) — criterion for switch rules.
    /// Deliberately an open id, not an enum: classes are level data.
    TrainClass
);

#[cfg(test)]
mod tests {
    use super::segment_lengths::*;
    use super::*;

    /// The frozen table — if this test ever fails, someone changed values
    /// that must not change (see module docs).
    #[test]
    fn segment_lengths_are_frozen() {
        assert_eq!(STRAIGHT, Len(1000));
        assert_eq!(DIAGONAL, Len(1414));
        assert_eq!(CURVE_45, Len(1207));
        assert_eq!(CURVE_90, Len(1000));
        assert_eq!(MAX_SPEED_EXCLUSIVE, 500);
    }

    #[test]
    fn speed_times_ticks_is_length() {
        assert_eq!(Speed(120) * TickDelta(3), Len(360));
    }

    #[test]
    fn tick_arithmetic() {
        assert_eq!(Tick(10) + TickDelta(5), Tick(15));
        assert_eq!(Tick(10) - Tick(4), TickDelta(6));
    }
}
