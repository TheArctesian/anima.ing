#[cfg(target_os = "linux")]
mod automatic;
#[cfg(target_os = "linux")]
mod boot;
#[cfg(target_os = "linux")]
mod command;
#[cfg(target_os = "linux")]
mod download;
#[cfg(target_os = "linux")]
mod feed;
#[cfg(target_os = "linux")]
mod state;
#[cfg(target_os = "linux")]
mod update;

#[cfg(target_os = "linux")]
use std::{path::Path, process::ExitCode};

#[cfg(target_os = "linux")]
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{0}")]
    Refused(&'static str),
    #[error("The update could not access local storage. Check available space and retry.")]
    Io(#[from] std::io::Error),
    #[error("The update metadata is invalid.")]
    Json(#[from] serde_json::Error),
    #[error("The update server could not be reached securely. Check connectivity and retry.")]
    Network(#[from] reqwest::Error),
}

#[cfg(target_os = "linux")]
type Result<T> = std::result::Result<T, Error>;

#[cfg(target_os = "linux")]
fn main() -> ExitCode {
    let outcome = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(Error::Io)
        .and_then(|runtime| runtime.block_on(run()));
    match outcome {
        Ok(value) => {
            println!("{}", serde_json::json!({"ok":true,"value":value}));
            ExitCode::SUCCESS
        }
        Err(error) => {
            println!(
                "{}",
                serde_json::json!({"ok":false,"error":error.to_string()})
            );
            ExitCode::FAILURE
        }
    }
}

#[cfg(target_os = "linux")]
async fn run() -> Result<serde_json::Value> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if let [operation, manifest, flag, key] = args.as_slice()
        && operation == "verify"
        && flag == "--key"
    {
        let bytes = state::read_bounded(Path::new(manifest), 65536)?;
        return Ok(serde_json::to_value(
            feed::verify(&bytes, Path::new(key)).await?,
        )?);
    }
    let [action] = args.as_slice() else {
        return Err(Error::Refused(
            "Choose status, check, automatic, download, activate, rollback, health or failure.",
        ));
    };
    if !nix::unistd::Uid::effective().is_root() {
        return Err(Error::Refused("Local owner authentication required."));
    }
    state::prepare()?;
    if action == "status" {
        return state::status();
    }
    let lock = state::lock()?;
    lock.try_lock()
        .map_err(|_| Error::Refused("An update is already running. Wait for it to finish."))?;
    update::dispatch(action).await
}

#[cfg(not(target_os = "linux"))]
fn main() -> std::process::ExitCode {
    println!("{{\"ok\":false,\"error\":\"Anima OS updates are supported only on Linux.\"}}");
    std::process::ExitCode::FAILURE
}
