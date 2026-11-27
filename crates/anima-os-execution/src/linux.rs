mod broker;
mod builds;
mod command;
mod control;
mod digests;
mod preview_broker;
mod previews;
mod provider_broker;
mod provider_login;
mod provisioning;
mod recovery;
mod requests;
mod source_jobs;
use requests::{Request, Stream};
mod transport;
mod work_policy;
mod workspace;

use serde_json::json;
use std::process::ExitCode;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt};

type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
enum Error {
    #[error("Another operation owns the source-job journal.")]
    SourceJournalBusy,
    #[error("Another operation owns this build attempt.")]
    BuildAttemptBusy,
    #[error("The build ran out of memory.")]
    BuildOutOfMemory,
    #[error("Another operation owns this workspace.")]
    WorkspaceBusy,
    #[error("{0}")]
    Refused(&'static str),
    #[error("Workspace storage or its runtime is unavailable.")]
    Io(#[from] std::io::Error),
    #[error("Workspace state is invalid.")]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Admission(#[from] anima_os_maintenance::Error),
}

pub fn run() -> ExitCode {
    let stdio = std::env::args_os().len() == 1;
    let result = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(Error::Io)
        .and_then(|runtime| {
            let result = runtime.block_on(entry());
            if stdio {
                // Cancelled blocking stdio must not keep a completed command alive.
                runtime.shutdown_background();
            }
            result
        });
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("anima-os-execution: {error}");
            ExitCode::FAILURE
        }
    }
}

async fn entry() -> Result<()> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if let [operation, identity] = args.as_slice()
        && matches!(
            operation.as_str(),
            "source-job-worker" | "source-job-finish"
        )
    {
        return source_jobs::dispatch(operation, identity).await;
    }
    if args.as_slice() == ["source-job-cleanup"] {
        return source_jobs::run_cleanup().await;
    }
    if args.as_slice() == ["source-job-broker"] {
        return source_jobs::broker::run().await;
    }
    if args.as_slice() == ["build-worker"] {
        return builds::worker::run().await;
    }
    if args.as_slice() == ["workspace-broker"] {
        owner("Workspace lifecycle requires the root owner.")?;
        return provisioning::broker::run().await;
    }
    if args.as_slice() == ["broker"] {
        owner("Local owner authorization is required.")?;
        return broker::run().await;
    }
    if args.as_slice() == ["build-watch"] {
        owner("Build lease enforcement requires the root owner.")?;
        tokio::try_join!(builds::lease::watch(), source_jobs::watch_loop())?;
        return Ok(());
    }
    if args.as_slice() == ["hosting-pressure"] {
        owner("Hosting pressure control requires the root owner.")?;
        return anima_os_maintenance::pressure::watch()
            .await
            .map_err(Error::from);
    }
    if args.as_slice() == ["recover"] {
        owner("Local owner authorization is required.")?;
        return recovery::run().await;
    }
    if !args.is_empty() {
        return Err(Error::Refused("Unsupported execution command."));
    }
    serve().await
}

fn owner(refusal: &'static str) -> Result<()> {
    if !nix::unistd::Uid::effective().is_root() {
        return Err(Error::Refused(refusal));
    }
    nix::sys::stat::umask(nix::sys::stat::Mode::from_bits_truncate(0o077));
    Ok(())
}

async fn serve() -> Result<()> {
    owner("Local owner authorization is required.")?;
    let mut input = tokio::io::BufReader::new(tokio::io::stdin());
    let mut frame = Vec::new();
    tokio::time::timeout(
        std::time::Duration::from_secs(5),
        (&mut input).take(65537).read_until(b'\n', &mut frame),
    )
    .await
    .map_err(|_| Error::Refused("The workspace request frame timed out."))??;
    if frame.len() > 65536 || frame.last() != Some(&b'\n') {
        return Err(Error::Refused(
            "The workspace request exceeded its frame limit.",
        ));
    }
    let request: Request = serde_json::from_slice(&frame)?;
    let result = match request {
        Request::Stream(Stream::WorkspaceHandoffProjectExport { receipt }) => {
            return workspace::handoff::project::stream(receipt).await;
        }
        Request::Stream(Stream::WorkspaceHandoffProjectRestore { import }) => {
            return match workspace::handoff::restore::receive(import, input).await {
                Ok(()) => Ok(()),
                Err(error) => reply(Err(error)).await,
            };
        }
        Request::Stream(Stream::BuildExport {
            attempt,
            archive_digest,
            manifest_digest,
        }) => {
            return builds::export(attempt, &archive_digest, &manifest_digest).await;
        }
        Request::Stream(Stream::Connect {
            workspace,
            generation,
        }) => {
            return transport::connect(workspace, generation, input).await;
        }
        Request::Stream(Stream::Preview {
            endpoint,
            revision,
            principal,
        }) => {
            let lease = previews::resolve(endpoint, revision, principal)?;
            return transport::preview(lease, input).await;
        }
        Request::Operation(operation) => operation.execute().await,
    };
    reply(result).await
}

async fn reply(result: Result<serde_json::Value>) -> Result<()> {
    let reply = match &result {
        Ok(value) => json!({"ok":true,"value":value}),
        Err(error) => json!({"ok":false,"error":error.to_string()}),
    };
    let mut bytes = serde_json::to_vec(&reply)?;
    bytes.push(b'\n');
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        let mut output = tokio::io::stdout();
        output.write_all(&bytes).await?;
        output.flush().await
    })
    .await
    .map_err(|_| Error::Refused("The workspace reply timed out."))??;
    result.map(drop)
}
