//! Durable delivery of Code mutations to one managed runner generation.
//! The local root caller must have already authenticated the person and workspace grant.
pub(super) mod attention;
mod exchange;
pub(super) mod inventory;
mod store;

use super::{Error, Result, workspace};
use agent_threads::control::{Action, Call, Outcome};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use store::{Receipt, Store};
use uuid::Uuid;

pub(super) use agent_threads::managed::Command;

pub(super) async fn deliver(command: Command) -> Result<Value> {
    authorized(command, None).await
}

pub(super) async fn authorized(
    command: Command,
    deadline: Option<tokio::time::Instant>,
) -> Result<Value> {
    validate(&command)?;
    let fingerprint = super::digests::hex(&Sha256::digest(serde_json::to_vec(&command)?));
    let store = Store::open(command.workspace)?;
    if let Some(held) = store.read(command.request)? {
        if held.fingerprint != fingerprint {
            return Err(Error::Refused(
                "This command identity already belongs to a different request.",
            ));
        }
        return Ok(view(&held));
    }
    check_deadline(deadline)?;
    let container = workspace::connection(command.workspace, command.generation).await?;
    let expected = command.clone();
    let thread = matches!(&command.action, Action::Start { .. }).then_some(command.request);
    let action = match thread {
        Some(thread) => Action::StartManaged {
            thread,
            start: Box::new(command.action),
        },
        None => command.action,
    };
    let mut receipt = Receipt {
        schema: 1,
        request: command.request,
        workspace: command.workspace,
        generation: command.generation,
        principal: command.principal,
        fingerprint,
        result: None,
        thread,
    };
    // Publish intent before the first byte can reach the runner. A crash thereafter
    // cannot distinguish delivery from nondelivery, so retries never redispatch it.
    store.reserve(&receipt)?;
    check_deadline(deadline)?;
    let result = exchange::call(&container, Call { id: 1, action }).await?;
    if !expected.accepts(&result) {
        return Err(Error::Refused(
            "The runner response does not match this managed command.",
        ));
    }
    receipt.result = Some(result);
    store.save(&receipt)?;
    Ok(view(&receipt))
}

pub(super) async fn status(workspace: Uuid, request: Uuid, principal: Uuid) -> Result<Value> {
    let store = Store::open(workspace)?;
    let receipt = store
        .read(request)?
        .ok_or(Error::Refused("No receipt exists for this command."))?;
    if receipt.principal != principal {
        return Err(Error::Refused("This command belongs to another principal."));
    }
    let mut view = view(&receipt);
    if receipt.result.is_none() && receipt.thread.is_some() {
        view["observation"] = observe(&receipt).await;
    }
    Ok(view)
}

async fn observe(receipt: &Receipt) -> Value {
    let observed = async {
        let container = workspace::connection(receipt.workspace, receipt.generation).await?;
        exchange::call(
            &container,
            Call {
                id: 1,
                action: Action::List,
            },
        )
        .await
    }
    .await;
    match observed {
        Ok(Outcome::Threads { threads, .. }) => match threads
            .into_iter()
            .find(|row| Some(row.id) == receipt.thread)
        {
            Some(row) => json!({"state":"thread-observed","thread":row,
                "firstPromptDelivery":"unconfirmed"}),
            None => json!({"state":"not-observed","safeToRedispatch":false}),
        },
        _ => json!({"state":"unavailable","safeToRedispatch":false}),
    }
}

fn view(receipt: &Receipt) -> Value {
    json!({"request":receipt.request,"workspace":receipt.workspace,
        "generation":receipt.generation,"thread":receipt.thread,
        "state":if receipt.result.is_some() { "completed" } else { "unresolved" },
        "result":receipt.result,"safeToRedispatch":false})
}

fn validate(command: &Command) -> Result<()> {
    if command.fits() {
        Ok(())
    } else {
        Err(Error::Refused(
            "This managed Code command is invalid or exceeds its size limit.",
        ))
    }
}

fn check_deadline(deadline: Option<tokio::time::Instant>) -> Result<()> {
    if deadline.is_some_and(|deadline| tokio::time::Instant::now() >= deadline) {
        Err(Error::Refused(
            "Managed Code delivery authorization expired.",
        ))
    } else {
        Ok(())
    }
}

pub(super) async fn snapshot(
    workspace: Uuid,
    generation: u64,
    thread: Uuid,
    after: u64,
) -> Result<Value> {
    let container = workspace::connection(workspace, generation).await?;
    let Outcome::ManagedTranscript { snapshot } = exchange::call(
        &container,
        Call {
            id: 1,
            action: Action::ManagedTranscript { thread, after },
        },
    )
    .await?
    else {
        return Err(Error::Refused("The runner transcript is unavailable."));
    };
    if snapshot.row.id != thread {
        return Err(Error::Refused("The runner snapshot names another thread."));
    }
    Ok(serde_json::to_value(snapshot)?)
}
