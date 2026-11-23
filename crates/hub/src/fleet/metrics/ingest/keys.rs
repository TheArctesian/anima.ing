//! `POST /daemon/v1/keys/fetch` and the `keys` member of a push (K8; K-E5 Section 2.1, R-147).
//! The daemon token is the credential; checks run in renewal order: bearer, size, JSON, device.
//! A missing, inactive or ungranted key is one `404 NO_KEY` from one lookup, so none can be probed.
//! The value leaves only in a `no-store` `200`, in a buffer wiped on drop, never logged (LOG-007).

use std::net::IpAddr;

use axum::{
    body::{Body, Bytes},
    extract::Request,
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse as _, Response},
};
use chrono::{DateTime, Utc};
use daemon_core::wire::{FetchedKey, KeyFetch, KeyVersion, KeysCurrent, RefusalCode};
use zeroize::Zeroizing;

use super::{
    Error,
    admission::{Admitted, Stop, admit},
    credential,
    refusals::Route,
    routes::{DaemonState, busy, client_of, read_body, unmounted},
    turns::Turn,
};
use crate::{
    fleet::{credentials::operational, events::ClientAddr},
    ids::DeviceId,
};

const BODY_LIMIT: usize = 4 * 1024;

/// A refusal goes to the event log without the token or the key's name.
pub(super) async fn serve_fetch(state: DaemonState, request: Request) -> Response {
    let Some(client) = client_of(&request) else {
        return unmounted();
    };
    let now = state.clock.now();
    let Some(address) = state.turns.address(client.0) else {
        return busy();
    };
    let (parts, body) = request.into_parts();
    match fetched(&state, (&parts.headers, body), (address, client), now).await {
        Ok(key) => answer(&key),
        Err(stop) => state.stopped(stop, (Route::KeysFetch, client), now).await,
    }
}

async fn fetched(
    state: &DaemonState,
    (headers, body): (&HeaderMap, Body),
    (address, client): (Turn<IpAddr>, ClientAddr),
    now: DateTime<Utc>,
) -> Result<FetchedKey, Stop> {
    let token =
        credential::bearer(headers).ok_or(Stop::Refused(RefusalCode::TokenRefused, None))?;
    let Admitted {
        device,
        turn: _turn,
        ..
    } = admit(state, &token, address, now).await?;
    drop(token);
    let refused = |code| Stop::Refused(code, Some(device));
    let asked = {
        let body = read_body(state, headers, body, BODY_LIMIT)
            .await
            .map_err(refused)?;
        serde_json::from_slice::<KeyFetch>(&body).map_err(|_| refused(RefusalCode::Malformed))?
    };
    if DeviceId::from(asked.device) != device {
        return Err(refused(RefusalCode::WrongDevice));
    }
    let _request = state.turns.request().await.ok_or(Stop::Busy)?;
    let found = operational::fetch(
        &state.db,
        &state.vault,
        (device, &asked.name),
        (now, client),
    )
    .await
    .map_err(|error| Stop::Failed(Error::Keys(error)))?;
    found.ok_or_else(|| refused(RefusalCode::NoKey))
}

/// Room is reserved first so the body never moves, and `Bytes` owns it so it is wiped on drop.
fn answer(key: &FetchedKey) -> Response {
    let room = 128 + key.name.len() + key.value.expose().len() * 2;
    let mut bytes = Zeroizing::new(Vec::with_capacity(room));
    if serde_json::to_writer(&mut *bytes, key).is_err() {
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }
    let headers = [
        (header::CONTENT_TYPE, "application/json"),
        (header::CACHE_CONTROL, "no-store"),
    ];
    (
        StatusCode::OK,
        headers,
        Body::from(Bytes::from_owner(bytes)),
    )
        .into_response()
}

/// `None` when either step fails: the batch stays stored, and a daemon told no keys keeps its cache.
pub(super) async fn current(
    state: &DaemonState,
    device: DeviceId,
    held: &[KeyVersion],
    now: DateTime<Utc>,
) -> Option<KeysCurrent> {
    let told = async {
        operational::record_held(&state.db, device, held, now).await?;
        operational::current_for(&state.db, device, now).await
    };
    match told.await {
        Ok(current) => Some(KeysCurrent { current }),
        Err(error) => {
            tracing::error!(%error, "the keys of a daemon batch could not be read");
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use daemon_core::wire::KeyValue;

    use super::*;

    #[test]
    fn r_147_a_fetched_key_is_answered_with_no_store() {
        let key = FetchedKey {
            name: "cloudflare.dns".to_owned(),
            version: 3,
            value: KeyValue::new(Zeroizing::new(b"value".to_vec())).unwrap(),
        };
        let answer = answer(&key);
        assert_eq!(answer.status(), StatusCode::OK);
        assert_eq!(answer.headers()[header::CACHE_CONTROL], "no-store");
    }
}
