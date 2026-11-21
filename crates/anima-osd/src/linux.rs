mod client;
mod probe;
mod protocol;
mod socket;
mod status;

use clap::{Parser, Subcommand};
use std::{path::PathBuf, process::ExitCode};

#[derive(Parser)]
#[command(about = "Anima OS local read-only control; no network listener or host mutations")]
struct Arguments {
    #[arg(long, default_value = "/run/anima-osd/control.sock")]
    socket: PathBuf,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Run the private Unix socket endpoint. Its parent must be owner-only.
    Serve {
        #[arg(long, default_value = "/run/current-system/sw/bin/systemctl")]
        systemctl: PathBuf,
    },
    /// Read local host facts. This does not establish hub connectivity or work readiness.
    Status,
    /// Read local monitoring/link process state and service-namespace root mount state.
    Diagnostics,
}

pub fn run() -> ExitCode {
    let arguments = Arguments::parse();
    let result = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(Error::Io)
        .and_then(|runtime| runtime.block_on(execute(arguments)));
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("anima-osd: {error}");
            ExitCode::FAILURE
        }
    }
}

async fn execute(arguments: Arguments) -> Result<(), Error> {
    match arguments.command {
        Command::Serve { systemctl } => socket::serve(&arguments.socket, &systemctl).await,
        Command::Status => client::request(&arguments.socket, protocol::Operation::Status).await,
        Command::Diagnostics => {
            client::request(&arguments.socket, protocol::Operation::Diagnostics).await
        }
    }
}

#[derive(Debug, thiserror::Error)]
enum Error {
    #[error("local control I/O failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("local control JSON failed: {0}")]
    Json(#[from] serde_json::Error),
    #[error("{0}")]
    Policy(&'static str),
}
