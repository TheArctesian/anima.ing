//! The order of a rollout (ROL-023, ROL-025). The hub itself is not part of a rollout (R-139).

use super::model::{Machine, Reach};
use crate::ids::DeviceId;

/// A quarter of the machines, and at least one (ROL-023).
#[must_use]
pub const fn step_size(machines: usize) -> usize {
    let quarter = machines / 4;
    if quarter == 0 { 1 } else { quarter }
}

/// `None` when no included machine qualifies; the operator then selects one (ROL-025).
#[must_use]
pub fn first_machine(included: &[Machine]) -> Option<DeviceId> {
    included
        .iter()
        .find(|machine| {
            machine.reach == Reach::Connected
                && !machine.with_hub
                && machine.said.is_some_and(|said| !said.busy)
        })
        .map(|machine| machine.device)
}

/// `first` alone, then `others` in steps (ROL-023); a repeat of `first` in `others` is left out.
#[must_use]
pub fn steps(first: DeviceId, others: &[DeviceId]) -> Vec<Vec<DeviceId>> {
    let rest: Vec<DeviceId> = others
        .iter()
        .copied()
        .filter(|other| *other != first)
        .collect();
    let size = step_size(rest.len() + 1);
    let mut all = vec![vec![first]];
    all.extend(rest.chunks(size).map(<[DeviceId]>::to_vec));
    all
}
