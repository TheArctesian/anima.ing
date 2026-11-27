//! Observe installed Hosting setup and the local service.
use std::{
    fs, io,
    os::unix::fs::{FileTypeExt as _, MetadataExt as _},
};

use serde_json::{Value, json};

use super::command;

pub(super) async fn observe() -> Value {
    let (settings, service) = tokio::join!(
        command::run(
            "/run/current-system/sw/bin/anima-hosting-settings",
            &["--status", "/persist/anima-applications"],
            None,
            3,
        ),
        command::unit_state("anima-applications.service"),
    );
    let setup = settings
        .ok()
        .and_then(|text| serde_json::from_str::<String>(&text).ok());
    let socket = match fs::symlink_metadata("/run/anima-applications/control.sock") {
        Ok(metadata) if metadata.file_type().is_socket() && metadata.uid() == 0 => Some(true),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Some(false),
        _ => None,
    };
    json!({"setup":setup,"service":service,"socket":socket})
}
