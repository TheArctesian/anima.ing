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
mod remote;
#[cfg(target_os = "linux")]
mod state;
#[cfg(target_os = "linux")]
mod update;
#[cfg(target_os = "linux")]
mod window;

#[cfg(target_os = "linux")]
use std::{path::Path, process::ExitCode};

#[cfg(target_os = "linux")]
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Maintenance(#[from] anima_os_maintenance::Error),
    #[error("The update changed. Review it again before restarting.")]
    ReviewChanged,
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
            let mut value = serde_json::json!({"ok":false,"error":error.to_string()});
            if matches!(error, Error::ReviewChanged) {
                value["code"] = serde_json::json!("REVIEW_CHANGED");
            }
            println!("{value}");
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
    if !nix::unistd::Uid::effective().is_root() {
        return Err(Error::Refused("Local owner authentication required."));
    }
    if let Some(outcome) = window::dispatch(&args).await {
        return outcome;
    }
    let (action, review) = match args.as_slice() {
        [action] => (action.as_str(), None),
        [action, flag, revision]
            if flag == "--review"
                && matches!(
                    action.as_str(),
                    "activate"
                        | "rollback"
                        | "remote-activate"
                        | "remote-rollback"
                        | "remote-activate-run"
                        | "remote-rollback-run"
                ) =>
        {
            (action.as_str(), Some(revision.as_str()))
        }
        _ => {
            return Err(Error::Refused(
                "Choose status, check, automatic, download, activate, rollback, schedule-activation, cancel-activation, health or failure.",
            ));
        }
    };
    state::prepare()?;
    if let Some(action) = action.strip_prefix("remote-") {
        return remote::dispatch(action, review).await;
    }
    if action == "status" {
        return state::reviewed_status();
    }
    let lease = maintenance(action).await?;
    let _lock = state::writer()?;
    if let Some(review) = review {
        state::require_review(action, review)?;
    }
    update::dispatch(action, lease).await
}

#[cfg(not(target_os = "linux"))]
fn main() -> std::process::ExitCode {
    println!("{{\"ok\":false,\"error\":\"Anima OS updates are supported only on Linux.\"}}");
    std::process::ExitCode::FAILURE
}

#[cfg(target_os = "linux")]
async fn maintenance(action: &str) -> Result<Option<anima_os_maintenance::Lease>> {
    let operation = match action {
        "activate" => anima_os_maintenance::Operation::Activate,
        "failure" if update::retry_needed()? => anima_os_maintenance::Operation::Activate,
        "rollback" => anima_os_maintenance::Operation::Rollback,
        _ => return Ok(None),
    };
    Ok(Some(anima_os_maintenance::Lease::prepare(operation).await?))
}
