//! Each route document has its own generated type, so a route renders from its JSON.

use serde::Serialize;
use serde_json::Value;

use crate::output::{Cell, Table, Tone};

fn text(route: &Value, field: &str) -> String {
    match route.get(field) {
        Some(Value::String(text)) => text.to_lowercase().replace('_', " "),
        Some(Value::Null) | None => "-".to_owned(),
        Some(other) => other.to_string(),
    }
}

fn state(route: &Value) -> Cell {
    match route.get("waitsFor") {
        Some(Value::String(waits)) => Cell::toned(
            &format!("waits for {}", waits.to_lowercase().replace('_', " ")),
            Tone::Warning,
        ),
        _ => Cell::toned("serves", Tone::Good),
    }
}

pub fn table<T: Serialize>(routes: &[T]) -> Table {
    let mut table = Table::new(vec![
        "ID", "NAME", "KIND", "ACCESS", "DEVICE", "PORT", "LISTEN", "PROOF", "STATE",
    ]);
    for route in routes {
        let route = serde_json::to_value(route).unwrap_or_default();
        table.row(vec![
            text(&route, "id").into(),
            text(&route, "name").into(),
            text(&route, "kind").into(),
            text(&route, "access").into(),
            text(&route, "deviceId").into(),
            text(&route, "port").into(),
            text(&route, "listenPort").into(),
            text(&route, "proof").into(),
            state(&route),
        ]);
    }
    table
}

pub fn record<T: Serialize>(route: &T) -> Table {
    let route = serde_json::to_value(route).unwrap_or_default();
    let mut table = Table::record();
    for (label, field) in [
        ("ID", "id"),
        ("Name", "name"),
        ("Kind", "kind"),
        ("Access", "access"),
        ("Application", "applicationId"),
        ("Device", "deviceId"),
        ("Port", "port"),
        ("Port of the hub", "listenPort"),
        ("Protocol", "protocol"),
        ("Redirect to", "redirectTo"),
        ("Proof", "proof"),
        ("Failed reads", "failedReads"),
        ("TXT record", "proofRecordName"),
        ("TXT value", "proofRecordValue"),
        ("Machine consent", "machineConsent"),
        ("Certificate until", "certificateNotAfter"),
        ("Requests", "requests"),
        ("Bytes in", "bytesIn"),
        ("Bytes out", "bytesOut"),
        ("Refused", "refused"),
    ] {
        table.pair(label, text(&route, field));
    }
    table.pair("State", state(&route));
    table
}
