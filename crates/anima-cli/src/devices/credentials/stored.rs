use crate::output::{Cell, Table, Tone};

#[must_use]
pub fn table(protocol: &str, enabled: bool, port: i64, credential_stored: bool) -> Table {
    let mut table = Table::record();
    table.pair("Access", protocol);
    table.pair("Enabled", if enabled { "yes" } else { "no" });
    table.pair("Port", port.to_string());
    let (text, tone) = if credential_stored {
        ("stored", Tone::Good)
    } else {
        ("none", Tone::Muted)
    };
    table.pair("Credential", Cell::toned(text, tone));
    table
}
