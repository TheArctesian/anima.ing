use axum::{
    http::StatusCode,
    response::{IntoResponse as _, Response},
};
use daemon_core::wire::{Claim, RefusalCode, Switch};
use serde::Deserialize;
use serde_json::Value;
use uuid::Uuid;

use crate::{
    fleet::agent_threads::remote::{self, Error, Pair},
    work::jobs::{self, Claimed},
};

use super::super::{
    refusals,
    remote_threads::{self, Holder},
    routes::DaemonState,
};
use super::ClientAddr;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ThreadOffer {
    threads: Option<Switch>,
    #[serde(default)]
    thread_targets: Vec<Pair>,
    types: Vec<String>,
    kinds: Vec<String>,
}

impl ThreadOffer {
    fn enabled(&self) -> bool {
        self.threads == Some(Switch::On)
            && self.types.iter().any(|kind| kind == "agent_thread")
            && self.kinds.iter().any(|kind| kind == "person")
    }
}

pub(super) async fn claim(
    state: &DaemonState,
    holder: Holder,
    (body, name): (Value, &str),
    client: ClientAddr,
) -> Response {
    issue(state, holder, (body, name), client)
        .await
        .unwrap_or_else(|error| remote_threads::response(&error))
}

async fn issue(
    state: &DaemonState,
    holder: Holder,
    (body, name): (Value, &str),
    client: ClientAddr,
) -> Result<Response, Error> {
    let base: Claim = parse(body.clone())?;
    let offer: ThreadOffer = parse(body)?;
    if base.device != Uuid::from(holder.device) {
        return Err(Error::Refused("WRONG_DEVICE"));
    }
    let (mut tx, at) = holder.locked(state).await?;
    let enabled = offer.enabled();
    let candidate = offer_thread(&mut tx, &holder, (&base, &offer), at).await?;
    let thread_at = candidate.map(|(_, _, timestamp)| timestamp);
    let regular = jobs::claim_in(
        &mut tx,
        &state.bus,
        (holder.device, name, &base),
        (client, state.clock.now(), thread_at),
    )
    .await
    .map_err(Error::Jobs)?;
    let answer = match (regular, candidate) {
        (Claimed::Job(job), _) => refusals::json(StatusCode::OK, &*job),
        (Claimed::Off, _) if !enabled => refusals::answer(RefusalCode::WorkOff),
        (Claimed::Nothing, Some((owner, launch, _))) => {
            let thread = remote::claim(&mut tx, owner, launch, at).await?;
            refusals::json(StatusCode::OK, &thread)
        }
        _ => StatusCode::NO_CONTENT.into_response(),
    };
    tx.commit().await?;
    Ok(answer)
}

fn parse<T: serde::de::DeserializeOwned>(body: Value) -> Result<T, Error> {
    serde_json::from_value(body).map_err(|_| Error::Refused("MALFORMED"))
}

async fn offer_thread(
    conn: &mut sqlx::PgConnection,
    holder: &Holder,
    (base, offer): (&Claim, &ThreadOffer),
    at: chrono::DateTime<chrono::Utc>,
) -> Result<Option<(remote::Owner, Uuid, chrono::DateTime<chrono::Utc>)>, Error> {
    let owner = match holder.owner(conn).await {
        Ok(owner) => owner,
        Err(Error::Refused("TOKEN_REFUSED")) if !offer.enabled() => return Ok(None),
        Err(error) => return Err(error),
    };
    let offered = if offer.enabled() {
        offer.thread_targets.as_slice()
    } else {
        &[]
    };
    remote::replace_targets(conn, owner, (offered, offer.enabled()), at).await?;
    if !offer.enabled() || base.unclean || base.free.slots == 0 {
        return Ok(None);
    }
    let candidate = remote::candidate(conn, owner, &base.holds, at).await?;
    Ok(candidate.map(|(launch, received)| (owner, launch, received)))
}

pub(super) async fn report(
    state: &DaemonState,
    holder: Holder,
    body: &Value,
) -> Result<Option<Response>, Error> {
    let (mut tx, at) = holder.locked(state).await?;
    let owner = match holder.owner(&mut tx).await {
        Ok(owner) => owner,
        Err(Error::Refused("TOKEN_REFUSED")) => return Ok(None),
        Err(error) => return Err(error),
    };
    let result = remote::report(&mut tx, owner, body, at).await?;
    tx.commit().await?;
    Ok(result.map(|result| refusals::json(StatusCode::OK, &result)))
}
