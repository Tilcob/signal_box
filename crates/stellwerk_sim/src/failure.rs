//! Deadlock detection: cycle search in the wait-for graph.
//!
//! Each blocked train waits on at most *one* other train (the holder or
//! reserver of the first block it needs), so the wait-for graph is a
//! functional graph — cycle detection is pointer chasing with visited
//! marks, no Tarjan needed. Self-waits (a train blocking itself via a ring
//! block) are *not* edges here; they end as `Stalled`, not `Deadlock`.

use crate::units::TrainId;
use std::collections::{BTreeMap, BTreeSet};

/// Finds a cycle, searching from ascending `TrainId`s; the returned cycle is
/// rotated so the smallest id comes first (deterministic error messages).
pub fn find_cycle(waits: &BTreeMap<TrainId, TrainId>) -> Option<Vec<TrainId>> {
    let mut done: BTreeSet<TrainId> = BTreeSet::new();

    for &start in waits.keys() {
        if done.contains(&start) {
            continue;
        }
        // Walk the chain from `start`, remembering the order of this walk.
        let mut order: Vec<TrainId> = Vec::new();
        let mut seen_at: BTreeMap<TrainId, usize> = BTreeMap::new();
        let mut current = start;
        loop {
            if let Some(&pos) = seen_at.get(&current) {
                // Hit our own walk again: everything from `pos` is the cycle.
                let mut cycle = order[pos..].to_vec();
                let min_pos = cycle
                    .iter()
                    .enumerate()
                    .min_by_key(|(_, id)| **id)
                    .map(|(i, _)| i)
                    .expect("cycle is non-empty");
                cycle.rotate_left(min_pos);
                return Some(cycle);
            }
            if done.contains(&current) {
                // Joins an already-explored cycle-free tail.
                break;
            }
            seen_at.insert(current, order.len());
            order.push(current);
            match waits.get(&current) {
                Some(&next) => current = next,
                None => break, // chain ends at a movable train
            }
        }
        done.extend(order);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn waits(pairs: &[(u32, u32)]) -> BTreeMap<TrainId, TrainId> {
        pairs
            .iter()
            .map(|&(a, b)| (TrainId(a), TrainId(b)))
            .collect()
    }

    #[test]
    fn empty_and_chain_have_no_cycle() {
        assert_eq!(find_cycle(&waits(&[])), None);
        assert_eq!(find_cycle(&waits(&[(1, 2), (2, 3)])), None);
    }

    #[test]
    fn two_cycle() {
        assert_eq!(
            find_cycle(&waits(&[(2, 5), (5, 2)])),
            Some(vec![TrainId(2), TrainId(5)])
        );
    }

    #[test]
    fn cycle_with_tail_reports_only_the_cycle() {
        // 0 → 1 → 2 → 1: the cycle is [1, 2], the tail 0 is not part of it.
        assert_eq!(
            find_cycle(&waits(&[(0, 1), (1, 2), (2, 1)])),
            Some(vec![TrainId(1), TrainId(2)])
        );
    }

    #[test]
    fn cycle_starts_at_smallest_id() {
        assert_eq!(
            find_cycle(&waits(&[(7, 3), (3, 9), (9, 7)])),
            Some(vec![TrainId(3), TrainId(9), TrainId(7)])
        );
    }
}
