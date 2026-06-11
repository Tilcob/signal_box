use std::ops::{Add, Mul, Sub};

/// Fixed segment length
///
/// These values are fixed in M0 and are part of the deterministic simulation.
///
/// The values are never allowed to be changed, otherwise:
///     - Replay hashed get unguilty
///     - Old replays delivery other outcomings
///     - Personal bests cannot be compared
pub mod segment_lengths {
    use std::f32::consts::PI;
    use crate::units::Len;

    const STRAIGHT_LEN: Len = Len(1000);
    const DIAGONAL_LEN: Len = Len(1414);
    const CURVE_LEN: Len = Len(STRAIGHT_LEN.0 * PI as i64 / 2);

}

#[derive(Debug, Copy, Clone, PartialEq, PartialOrd)]
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

#[derive(Debug, Copy, Clone, PartialEq, PartialOrd)]
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

    fn sub(self, rhs: Self) -> Self::Output {
        TickDelta(self.0 - rhs.0)
    }
}

#[derive(Debug, Copy, Clone, PartialEq, PartialOrd)]
pub struct TickDelta(pub u64);

impl TickDelta {
    pub fn get_ticks(self) -> u64 {
        self.0
    }
}

#[derive(Debug, Copy, Clone, PartialEq, PartialOrd)]
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

#[derive(Debug, Copy, Clone, PartialEq, PartialOrd)]
pub struct TrainId(pub u32);

impl TrainId {
    pub fn get_id(self) -> u32 {
        self.0
    }
}

#[derive(Debug, Copy, Clone, PartialEq, PartialOrd)]
pub struct NodeId(pub u32);

impl NodeId {
    pub fn get_id(self) -> u32 {
        self.0
    }
}

#[derive(Debug, Copy, Clone, PartialEq, PartialOrd)]
pub struct EdgeId(pub u32);

impl EdgeId {
    pub fn get_id(self) -> u32 {
        self.0
    }
}

#[derive(Debug, Copy, Clone, PartialEq, PartialOrd)]
pub struct BlockId(pub u32);

impl BlockId {
    pub fn get_id(self) -> u32 {
        self.0
    }
}

#[derive(Debug, Copy, Clone, PartialEq, PartialOrd)]
pub struct SignalId(pub u32);

impl SignalId {
    pub fn get_id(self) -> u32 {
        self.0
    }
}

#[derive(Debug, Copy, Clone, PartialEq, PartialOrd)]
pub struct SwitchId(pub u32);

impl SwitchId {
    pub fn get_id(self) -> u32 {
        self.0
    }
}


