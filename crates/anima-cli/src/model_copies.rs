//! `anima model pull`, `anima model remove` and `anima model copies`: the copies of the model
//! variants on the machines (task L1-7; PULL-001, PULL-007, PULL-009).

pub mod copies;
pub mod documents;
pub mod pull;
pub mod remove;

pub use copies::CopiesArgs;
pub use pull::PullArgs;
pub use remove::RemoveArgs;

use crate::output::text::enum_name;

/// The state of a copy as a table shows it, with the progress of a pull that runs.
fn state_cell(state: &impl serde::Serialize, progress: Option<(f64, f64, Option<f64>)>) -> String {
    let name = enum_name(state);
    let Some((bytes, total, left)) = progress else {
        return name;
    };
    let percent = if total > 0.0 {
        bytes * 100.0 / total
    } else {
        0.0
    };
    match left {
        Some(seconds) => format!("{name} {percent:.0}%, {seconds:.0} s left"),
        None => format!("{name} {percent:.0}%"),
    }
}
