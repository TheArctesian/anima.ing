use axum::{
    Extension,
    extract::{Request, State},
    http::{HeaderMap, StatusCode, header::CONTENT_LENGTH},
    response::Response,
};
use daemon_core::{artifact, wire::RefusalCode};
use futures_util::TryStreamExt as _;
use tokio_util::io::StreamReader;

use super::super::{
    Error as DaemonError,
    admission::{Admitted, Stop, admit},
    credential,
    refusals::{self, Route},
    routes::{DaemonState, busy, client_of, unmounted},
};
use crate::{
    ids::DeviceId,
    work::artifacts::{Delivery, DeliveryError, Error, Offer, Pending, Receipt, Store},
};

pub(super) async fn serve(
    State(state): State<DaemonState>,
    store: Option<Extension<Store>>,
    request: Request,
) -> Response {
    let Some(client) = client_of(&request) else {
        return unmounted();
    };
    let Some(Extension(store)) = store else {
        return busy();
    };
    let (parts, body) = request.into_parts();
    let Some(token) = credential::bearer(&parts.headers) else {
        return state
            .stopped(
                Stop::Refused(RefusalCode::TokenRefused, None),
                (Route::ArtifactUpload, client),
                state.clock.now(),
            )
            .await;
    };
    let delivery = Delivery {
        store: &store,
        db: &state.db,
        clock: state.clock.clone(),
    };
    let input = StreamReader::new(body.into_data_stream().map_err(std::io::Error::other));
    let authenticate = || async {
        let address = state.turns.address(client.0).ok_or(Stop::Busy)?;
        tokio::time::timeout(
            state.deadlines.answer,
            admit(&state, &token, address, state.clock.now()),
        )
        .await
        .map_err(|_| Stop::Busy)?
    };
    let outcome = async {
        let admitted = authenticate().await?;
        let device = admitted.device;
        drop(admitted);
        let offer =
            offer(&parts.headers).ok_or(Stop::Refused(RefusalCode::Malformed, Some(device)))?;
        let pending = delivery
            .prepare(device, offer, input)
            .await
            .map_err(|error| refused(device, error))?;
        let admitted = authenticate().await?;
        if admitted.device != device {
            return Err(Stop::Refused(RefusalCode::TokenRefused, None));
        }
        retain(&state, store.clone(), pending, admitted).await
    }
    .await;
    match outcome {
        Ok(receipt) => refusals::json(StatusCode::OK, &receipt),
        Err(stop) => {
            state
                .stopped(stop, (Route::ArtifactUpload, client), state.clock.now())
                .await
        }
    }
}

async fn retain(
    state: &DaemonState,
    store: Store,
    pending: Pending,
    admitted: Admitted,
) -> Result<Receipt, Stop> {
    let db = state.db.clone();
    let clock = state.clock.clone();
    // Once authorized for the commit, keep the ownership guards until storage finishes
    // even if the client disconnects before receiving its acknowledgement.
    tokio::spawn(async move {
        let delivery = Delivery {
            store: &store,
            db: &db,
            clock,
        };
        let result = delivery.retain(pending).await;
        let device = admitted.device;
        drop(admitted);
        result.map_err(|error| refused(device, error))
    })
    .await
    .map_err(|_| Stop::Busy)?
}

fn offer(headers: &HeaderMap) -> Option<Offer> {
    let field = |key: &str| {
        let mut values = headers.get_all(key).iter();
        let value = values.next()?.to_str().ok()?;
        (values.next().is_none() && value.len() <= 71).then_some(value)
    };
    let offer = Offer {
        job: field(artifact::JOB)?.parse().ok()?,
        attempt: field(artifact::ATTEMPT)?.parse().ok()?,
        bytes: field(artifact::BYTES)?.parse().ok()?,
        archive_digest: field(artifact::ARCHIVE)?.to_owned(),
        manifest_digest: field(artifact::MANIFEST)?.to_owned(),
    };
    if headers.contains_key(CONTENT_LENGTH)
        && !field(CONTENT_LENGTH.as_str())?
            .parse::<u64>()
            .is_ok_and(|bytes| bytes == 0 || bytes == offer.bytes)
    {
        return None;
    }
    Some(offer)
}

fn refused(device: DeviceId, error: DeliveryError) -> Stop {
    let code = match error {
        DeliveryError::NotHolder | DeliveryError::Storage(Error::Conflict) => {
            RefusalCode::NotHolder
        }
        DeliveryError::Off => RefusalCode::WorkOff,
        DeliveryError::Storage(Error::Invalid) => RefusalCode::Malformed,
        DeliveryError::Jobs(error) => return Stop::Failed(DaemonError::Jobs(error)),
        DeliveryError::Unavailable | DeliveryError::Storage(_) => return Stop::Busy,
    };
    Stop::Refused(code, Some(device))
}
