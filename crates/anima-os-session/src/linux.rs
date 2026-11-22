mod account;
mod command;
mod diagnostics;
mod display;
mod enrollment;
mod http;
mod inventory;
mod network;
mod session;

use serde::Deserialize;
use serde_json::{Value, json};
use std::{process::ExitCode, sync::Arc};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use zeroize::Zeroizing;

type Result<T> = std::result::Result<T, Error>;
#[derive(Debug, thiserror::Error)]
enum Error {
    #[error("The local operation could not finish. Check the connection and retry.")]
    Io(#[from] std::io::Error),
    #[error("The operation returned invalid information.")]
    Json(#[from] serde_json::Error),
    #[error("Could not contact the account service. Check the connection and retry.")]
    Network(#[from] reqwest::Error),
    #[error("{0}")]
    Refused(&'static str),
    #[error("{0}")]
    Update(String),
}

pub fn run() -> ExitCode {
    let result = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .map_err(Error::Io)
        .and_then(|runtime| runtime.block_on(execute()));
    if let Err(error) = result {
        eprintln!("anima-os-session: {error}");
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    id: String,
    op: String,
    #[serde(default = "empty")]
    args: Value,
}
fn empty() -> Value {
    json!({})
}

async fn execute() -> Result<()> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.first().is_some_and(|arg| arg == "display-auto") {
        return display::auto().await;
    }
    if args.first().is_some_and(|arg| arg == "display-list") {
        return display::print_list().await;
    }
    if !nix::unistd::Uid::effective().is_root() {
        return Err(Error::Refused("Local owner authentication required."));
    }
    nix::sys::stat::umask(nix::sys::stat::Mode::from_bits_truncate(0o077));
    if args.first().is_some_and(|arg| arg.starts_with("display-")) {
        return display::cli(&args).await;
    }
    if !args.is_empty() {
        return Err(Error::Refused("Unsupported command."));
    }
    serve().await
}

async fn serve() -> Result<()> {
    let session = Arc::new(session::Session::default());
    let output = Arc::new(tokio::sync::Mutex::new(tokio::io::stdout()));
    write(&output, &json!({"id":"unlock","ok":true,"value":{}})).await?;
    let mut input = BufReader::new(tokio::io::stdin());
    let mut tasks = tokio::task::JoinSet::new();
    let permits = Arc::new(tokio::sync::Semaphore::new(8));
    loop {
        let mut bytes = Zeroizing::new(Vec::new());
        let read = tokio::io::AsyncReadExt::take(&mut input, 65537)
            .read_until(b'\n', &mut bytes)
            .await?;
        if read == 0 {
            break;
        }
        if bytes.len() > 65536 || bytes.last() != Some(&b'\n') {
            return Err(Error::Refused("Request exceeded its frame limit."));
        }
        let request: Request = serde_json::from_slice(&bytes)?;
        if request.id.len() > 128 || request.op.len() > 64 {
            return Err(Error::Refused("Invalid request identity."));
        }
        let Ok(permit) = permits.clone().try_acquire_owned() else {
            write(&output,&json!({"id":request.id,"ok":false,"error":"Wait for the current operations to finish."})).await?;
            continue;
        };
        let (session, output) = (session.clone(), output.clone());
        tasks.spawn(async move {
            let _permit = permit;
            let result = session.dispatch(&request.op, request.args).await;
            let reply = match result {
                Ok(value) => json!({"id":request.id,"ok":true,"value":value}),
                Err(error) => json!({"id":request.id,"ok":false,"error":error.to_string()}),
            };
            if write(&output, &reply).await.is_err() {
                eprintln!("anima-os-session: reply recipient disconnected");
            }
        });
        while tasks.try_join_next().is_some() {}
    }
    while tasks.join_next().await.is_some() {}
    session.revert().await?;
    Ok(())
}

async fn write(output: &tokio::sync::Mutex<tokio::io::Stdout>, value: &Value) -> Result<()> {
    let mut bytes = serde_json::to_vec(value)?;
    bytes.push(b'\n');
    let mut output = output.lock().await;
    output.write_all(&bytes).await?;
    output.flush().await?;
    Ok(())
}
