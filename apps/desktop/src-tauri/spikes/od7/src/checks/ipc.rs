//! Check 2 (WEB-007): a device page cannot run a Tauri command; the proof is the labels the command records.

use std::time::Duration;

use super::device::{self, STORE_IPC};
use super::{Harness, Results};
use crate::event::{Event, Journal, is_report};

pub async fn check(harness: &Harness, journal: &mut Journal, results: &mut Results) {
    let from = journal.mark();
    if let Err(reason) = device::open(
        harness,
        "web-ipc",
        Some(STORE_IPC),
        harness.http_url("/ipc"),
    ) {
        results.record(false, "ipc-device-denied", reason);
        return;
    }
    let report = journal
        .until(from, Duration::from_secs(12), |e| {
            is_report(e, "http", "ipc", None)
        })
        .await;
    // A raw message may be handled after the page has reported.
    journal.settle(Duration::from_secs(1)).await;
    let reached: Vec<String> = journal
        .since(
            from,
            |e| matches!(e, Event::Invoked { label, .. } if label != "main"),
        )
        .into_iter()
        .filter_map(|e| match e {
            Event::Invoked { label, from } => Some(format!("{label} ({from})")),
            _ => None,
        })
        .collect();
    let page = match &report {
        Some(Event::Report { body, .. }) => body.to_string(),
        _ => "no report from the device page".to_owned(),
    };
    results.record(
        report.is_some() && reached.is_empty(),
        "ipc-device-denied",
        format!("command calls from device labels: {reached:?}; page saw {page}"),
    );
}
