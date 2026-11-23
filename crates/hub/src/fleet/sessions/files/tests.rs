//! In-crate tests, so the waits and the loopback deny list stay crate-private.

mod bench;
mod ending;
mod opening;
mod protocol;
mod refusals;
mod route;
mod server;
mod sftp;
mod socket;
mod tickets;
mod transfer;
mod transfers;

use std::time::Duration;

const WAIT: Duration = Duration::from_secs(5);

fn device_path(text: &str) -> super::protocol::DevicePath {
    let message = format!(r#"{{"type":"list","path":{}}}"#, serde_json::json!(text));
    match super::protocol::parse(&message) {
        Ok(super::protocol::Request::List(path)) => path,
        other => panic!("{other:?}"),
    }
}
