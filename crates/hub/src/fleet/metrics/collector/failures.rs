//! One event when a device's collections start failing and one when they work again (D-10). Pure.
//!
//! The messages are fixed text with no host, address, command output or secret (LOG-007).
//! The reason of a failure goes to the process log with the device id only.

use crate::{
    fleet::events::{ActorKind, NewEvent, Severity, Source},
    ids::DeviceId,
};

pub(super) const FAILING_MESSAGE: &str =
    "Metrics cannot be collected from the device; the hub tries again every 30 seconds";
pub(super) const WORKING_MESSAGE: &str = "Metrics are collected from the device again";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Change {
    /// The device's collection failed after working, or failed first after the hub started.
    StartedFailing,
    WorksAgain,
}

pub(super) const fn change(was_failing: bool, collected: bool) -> Option<Change> {
    match (was_failing, collected) {
        (false, false) => Some(Change::StartedFailing),
        (true, true) => Some(Change::WorksAgain),
        (false, true) | (true, false) => None,
    }
}

/// `events` stores the device under `Source::Metrics` (H-1).
pub(super) fn event(change: Change, device: DeviceId, name: String) -> NewEvent {
    let (severity, message) = match change {
        Change::StartedFailing => (Severity::Warning, FAILING_MESSAGE),
        Change::WorksAgain => (Severity::Info, WORKING_MESSAGE),
    };
    NewEvent {
        severity,
        source: Source::Metrics,
        device: Some((device, name)),
        message: message.to_owned(),
        actor: ActorKind::Hub,
    }
}
