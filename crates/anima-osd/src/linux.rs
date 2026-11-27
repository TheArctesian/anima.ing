mod client;
mod job_client;
mod jobs;
mod observation;
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
        #[arg(long)]
        state_directory: Option<PathBuf>,
        /// Publish read-only console observations at this boot-local path.
        #[arg(long)]
        console_snapshot: Option<PathBuf>,
    },
    /// Read local host facts. This does not establish hub connectivity or work readiness.
    Status,
    /// Read local monitoring/link process state and service-namespace root mount state.
    Diagnostics,
    /// Start or recover a persistent diagnostic operation by request UUID.
    JobStart { request_id: uuid::Uuid },
    /// Observe an operation by its original request UUID.
    JobGet { request_id: uuid::Uuid },
    /// List retained operations, optionally after a previous page cursor.
    JobList {
        #[arg(long)]
        after: Option<uuid::Uuid>,
    },
    /// Request cancellation; running probes finish bounded cleanup first.
    JobCancel { operation_id: uuid::Uuid },
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
        Command::Serve {
            systemctl,
            state_directory,
            console_snapshot,
        } => {
            socket::serve(
                &arguments.socket,
                &systemctl,
                state_directory.as_deref(),
                console_snapshot,
            )
            .await
        }
        Command::JobStart { request_id } => {
            job_client::request(
                &arguments.socket,
                jobs::schema::Operation::Start {
                    request_id,
                    action: jobs::schema::Action::Diagnostics,
                },
            )
            .await
        }
        Command::JobGet { request_id } => {
            job_client::request(
                &arguments.socket,
                jobs::schema::Operation::Get { request_id },
            )
            .await
        }
        Command::JobList { after } => {
            job_client::request(&arguments.socket, jobs::schema::Operation::List { after }).await
        }
        Command::JobCancel { operation_id } => {
            job_client::request(
                &arguments.socket,
                jobs::schema::Operation::Cancel { operation_id },
            )
            .await
        }
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
