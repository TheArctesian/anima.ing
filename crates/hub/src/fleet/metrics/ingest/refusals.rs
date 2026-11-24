//! How the daemon routes answer a refusal and write it to the event log (K8 Section 6, CR-15, CR-21).
//! Neither the answer nor the event holds a value from the request: no code, token or body text (LOG-007).

use std::sync::{Arc, Mutex, PoisonError};

use axum::{
    http::{StatusCode, header},
    response::{IntoResponse as _, Response},
};
use chrono::{DateTime, Utc};
use daemon_core::wire::{Refusal, RefusalCode};
use serde::Serialize;
use sqlx::PgPool;

use super::{Error, throttle::Throttle};
use crate::{
    fleet::{
        devices,
        events::{self, ActorKind, Bus, ClientAddr, NewEvent, Severity, Source},
    },
    ids::DeviceId,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Route {
    Enrol,
    Push,
    ThreadUpload,
    Renew,
    KeysFetch,
    WorkClaim,
    WorkReport,
    WorkSecret,
}

impl Route {
    const fn noun(self) -> &'static str {
        match self {
            Self::Enrol => "a daemon enrolment",
            Self::Push => "a daemon push",
            Self::ThreadUpload => "a daemon thread upload",
            Self::Renew => "a daemon token renewal",
            Self::KeysFetch => "a daemon key fetch",
            Self::WorkClaim => "a claim of work",
            Self::WorkReport => "a report of work",
            Self::WorkSecret => "a request for a secret of a job",
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(super) struct Refused {
    pub(super) route: Route,
    pub(super) code: RefusalCode,
    pub(super) client: ClientAddr,
    /// The device of a valid credential; `None` when the request had none.
    pub(super) device: Option<DeviceId>,
}

/// No cache keeps the answer, since `Enrolled` and `Renewed` hold a token.
pub(super) fn json(status: StatusCode, body: &impl Serialize) -> Response {
    let Ok(bytes) = serde_json::to_vec(body) else {
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    };
    let headers = [
        (header::CONTENT_TYPE, "application/json"),
        (header::CACHE_CONTROL, "no-store"),
    ];
    (status, headers, bytes).into_response()
}

pub(super) fn answer(code: RefusalCode) -> Response {
    let status = StatusCode::from_u16(code.status()).unwrap_or(StatusCode::BAD_REQUEST);
    json(status, &Refusal::from(code))
}

pub(super) fn sentence(refused: &Refused) -> String {
    format!(
        "Refused {} from {}: {}.",
        refused.route.noun(),
        refused.client.0,
        refused.code.as_str()
    )
}

pub(super) fn unrecorded_sentence(count: usize) -> String {
    format!("Left out {count} more daemon refusals from other addresses in the last minute.")
}

/// Writes at most once a minute per client address without a valid credential, and per device with one (CR-21).
#[derive(Clone)]
pub(super) struct Recorder {
    db: PgPool,
    bus: Bus,
    throttle: Arc<Mutex<Throttle>>,
}

impl Recorder {
    pub(super) fn new(db: PgPool, bus: Bus) -> Self {
        Self {
            db,
            bus,
            throttle: Arc::default(),
        }
    }

    fn due(&self, refused: &Refused, now: DateTime<Utc>) -> (bool, Option<usize>) {
        let mut throttle = self.throttle.lock().unwrap_or_else(PoisonError::into_inner);
        let own = match refused.device {
            Some(device) => throttle.admit_device(device, now),
            None => throttle.admit(refused.client.0, now),
        };
        (own, throttle.unrecorded(now).filter(|count| *count > 0))
    }

    pub(super) async fn record(&self, refused: &Refused, now: DateTime<Utc>) -> Result<(), Error> {
        let (own, unrecorded) = self.due(refused, now);
        if own {
            let device = match refused.device {
                Some(id) => self.named(id).await?,
                None => None,
            };
            self.write(device, sentence(refused), now).await?;
        }
        if let Some(count) = unrecorded {
            self.write(None, unrecorded_sentence(count), now).await?;
        }
        Ok(())
    }

    async fn named(&self, id: DeviceId) -> Result<Option<(DeviceId, String)>, Error> {
        match devices::get(&self.db, id).await {
            Ok(device) => Ok(Some((id, device.name))),
            Err(devices::Error::NotFound(_)) => Ok(None),
            Err(other) => Err(other.into()),
        }
    }

    async fn write(
        &self,
        device: Option<(DeviceId, String)>,
        message: String,
        now: DateTime<Utc>,
    ) -> Result<(), Error> {
        let event = NewEvent {
            severity: Severity::Warning,
            source: Source::Metrics,
            device,
            message,
            actor: ActorKind::Hub,
        };
        let mut conn = self.db.acquire().await?;
        events::record(&mut conn, &self.bus, now, event).await?;
        Ok(())
    }
}
