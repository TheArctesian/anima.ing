//! Workload receipts and optional usage for the installed console.
pub(in crate::linux) mod usage;

use crate::linux::{builds, workspace};
use serde_json::{Value, json};

pub(in crate::linux) async fn read(usage: bool) -> Value {
    let workspaces = if usage {
        workspace::inventory_with_usage().await
    } else {
        workspace::inventory()
    };
    json!({
        "workspaces":workspaces.ok(),
        "builds":builds::inventory(usage).ok(),
        "observation":"receipts-not-live-readiness",
        "admissionReconciled":anima_os_maintenance::recovery::ready().ok()
    })
}
