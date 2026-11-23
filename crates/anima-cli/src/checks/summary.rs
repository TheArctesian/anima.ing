//! What a command that changes a check prints.

use crate::output::{Cell, Table};

#[must_use]
pub fn state(status: &str, paused: bool) -> String {
    if paused {
        "PAUSED".to_owned()
    } else {
        status.to_owned()
    }
}

#[must_use]
pub fn written(name: &str, id: &str, kind: &str, state: &str) -> Table {
    let mut table = Table::record();
    table.pair("Name", name);
    table.pair("ID", id);
    table.pair("Kind", kind);
    table.pair("Status", Cell::status(state));
    table
}
