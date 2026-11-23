//! systemd service check (D-3, CHK-012); the result holds only fixed words, never output (LOG-007).

use super::{
    outcome::{Failure, ProbeResult},
    ssh::readable,
};
use crate::fleet::ssh::{CommandOutput, FixedCommand};

/// Static text only: the unit name is never part of the command.
pub(in crate::fleet::checks) fn command() -> FixedCommand {
    FixedCommand::new("systemctl")
        .arg("list-units")
        .arg("--all")
        .arg("--plain")
        .arg("--no-legend")
        .arg("--no-pager")
}

/// The unit types systemd knows; a name that ends in none of them is a service.
const SUFFIXES: [&str; 11] = [
    ".service",
    ".socket",
    ".target",
    ".timer",
    ".mount",
    ".automount",
    ".swap",
    ".path",
    ".slice",
    ".scope",
    ".device",
];

fn full_name(unit: &str) -> String {
    if SUFFIXES.iter().any(|suffix| unit.ends_with(suffix)) {
        unit.to_owned()
    } else {
        format!("{unit}.service")
    }
}

fn judge(load: &str, active: &str) -> ProbeResult {
    if load == "not-found" {
        return ProbeResult::failed(&Failure::UnitNotFound);
    }
    let class = match active {
        "active" => return ProbeResult::up_unmeasured(),
        "inactive" => "inactive",
        "failed" => "failed",
        "activating" => "activating",
        "deactivating" => "deactivating",
        "reloading" => "reloading",
        "maintenance" => "in maintenance",
        _ => "not active",
    };
    ProbeResult::failed(&Failure::UnitState(class))
}

/// Skips the bullet that marks a unit in a bad state when the output is not plain.
fn columns(row: &str) -> Option<(&str, &str, &str)> {
    let mut words = row
        .split_whitespace()
        .skip_while(|word| matches!(*word, "\u{25cf}" | "*"));
    Some((words.next()?, words.next()?, words.next()?))
}

pub(in crate::fleet::checks) fn read(output: &CommandOutput, unit: &str) -> ProbeResult {
    let text = match readable(output) {
        Ok(text) => text,
        Err(failure) => return ProbeResult::failed(&failure),
    };
    let wanted = full_name(unit);
    text.lines()
        .filter_map(columns)
        .find(|(name, ..)| *name == wanted)
        .map_or_else(
            || ProbeResult::failed(&Failure::UnitNotFound),
            |(_, load, active)| judge(load, active),
        )
}
