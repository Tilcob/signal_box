//! The three score axes. Material counts the *player* layout
//! only — designer-fixed track is free.

use crate::layout::{Layout, SignalKind};
use crate::units::Tick;

pub const COST_PIECE: u32 = 1;
pub const COST_SWITCH: u32 = 4;
pub const COST_BLOCK_SIGNAL: u32 = 2;
pub const COST_CHAIN_SIGNAL: u32 = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Score {
    /// Tick of the last arrival (throughput axis; smaller = better).
    pub throughput: Tick,
    /// Build cost of the player layout.
    pub material: u32,
    /// Sum of lateness ticks over all trains.
    pub lateness: u64,
}

pub fn material_cost(player: &Layout) -> u32 {
    let signals: u32 = player
        .signals
        .iter()
        .map(|s| match s.kind {
            SignalKind::Block => COST_BLOCK_SIGNAL,
            SignalKind::Chain => COST_CHAIN_SIGNAL,
        })
        .sum();
    player.pieces.len() as u32 * COST_PIECE + player.switches.len() as u32 * COST_SWITCH + signals
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grid::{Cell, Dir8};
    use crate::layout::{SignalDef, SwitchDef, TrackPiece};

    #[test]
    fn material_counts_per_cost_table() {
        let layout = Layout {
            pieces: vec![
                TrackPiece {
                    cell: Cell { x: 0, y: 0 },
                    a: Dir8::W,
                    b: Dir8::E,
                },
                TrackPiece {
                    cell: Cell { x: 1, y: 0 },
                    a: Dir8::W,
                    b: Dir8::E,
                },
            ],
            switches: vec![SwitchDef {
                cell: Cell { x: 2, y: 0 },
                stem: Dir8::W,
                branches: [Dir8::E, Dir8::NE],
                default_branch: 0,
                rules: vec![],
            }],
            signals: vec![
                SignalDef {
                    cell: Cell { x: 0, y: 0 },
                    at: Dir8::E,
                    kind: SignalKind::Block,
                    priority: 0,
                },
                SignalDef {
                    cell: Cell { x: 1, y: 0 },
                    at: Dir8::E,
                    kind: SignalKind::Chain,
                    priority: 0,
                },
            ],
        };
        assert_eq!(material_cost(&layout), 2 + 4 + 2 + 3);
    }
}
