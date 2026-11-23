//! Spec 6.9, WEB-001, WEB-002, WEB-008.
//! Writes take the caller's connection so the audit entry shares the transaction (LOG-005).
//! Messages in [`Error`] hold no input value (LOG-007).

pub mod graphql;
mod model;
mod rules;
mod store;
mod suggestion;
#[cfg(test)]
mod tests;

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use sqlx::{Connection, PgConnection, PgPool};
use uuid::Uuid;

use crate::{
    api_errors::field_names,
    fleet::checks::{self, config::CheckConfig},
    ids::{CheckId, DeviceId, WebInterfaceId},
};

pub use model::{
    Acceptance, Accepted, FieldError, Missing, OpenIn, Pin, Suggestion, Updated, WebInterface,
    WebInterfaceInput,
};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{0} not found")]
    NotFound(Missing),
    #[error("input is not valid: {}", field_names(.0))]
    Validation(Vec<FieldError>),
    #[error("database error")]
    Database(#[from] sqlx::Error),
    #[error("a stored web interface cannot be read")]
    Unreadable,
    #[error("the checks cannot be read")]
    Checks(#[source] checks::Error),
}

/// Each device's interfaces in creation order, in one query (API-005); a device with none is absent.
///
/// # Errors
/// `Database` when the query fails.
pub async fn for_devices(
    db: &PgPool,
    devices: &[DeviceId],
) -> Result<HashMap<DeviceId, Vec<WebInterface>>, Error> {
    Ok(group(store::for_devices(db, devices).await?))
}

fn group(interfaces: Vec<WebInterface>) -> HashMap<DeviceId, Vec<WebInterface>> {
    let mut grouped: HashMap<DeviceId, Vec<WebInterface>> = HashMap::new();
    for interface in interfaces {
        grouped.entry(interface.device).or_default().push(interface);
    }
    grouped
}

/// The HTTP checks of each device offered as interfaces (D-7). A device with none is absent.
///
/// # Errors
/// `Checks` when the checks cannot be read; `Database` when a query fails.
pub async fn suggestions(
    db: &PgPool,
    devices: &[DeviceId],
) -> Result<HashMap<DeviceId, Vec<Suggestion>>, Error> {
    let http = checks::http_urls(db, devices)
        .await
        .map_err(Error::Checks)?;
    let all_checks: Vec<CheckId> = http.values().flatten().map(|(check, _)| *check).collect();
    let dismissed = store::dismissed_among(db, &all_checks).await?;
    let interfaces = for_devices(db, devices).await?;
    Ok(http
        .into_iter()
        .filter_map(|(device, urls)| {
            let existing = interfaces.get(&device).map_or(&[][..], Vec::as_slice);
            let offered = suggestion::filter(&urls, existing, &dismissed);
            (!offered.is_empty()).then_some((device, offered))
        })
        .collect())
}

fn born(device: DeviceId, at: DateTime<Utc>) -> store::Born {
    store::Born {
        id: WebInterfaceId::from(Uuid::now_v7()),
        device,
        at,
    }
}

/// # Errors
/// `Validation` (`url`, `label`) when the input breaks a rule; `NotFound` when there is no such device.
pub async fn create(
    conn: &mut PgConnection,
    device: DeviceId,
    input: &WebInterfaceInput,
    now: DateTime<Utc>,
) -> Result<WebInterface, Error> {
    let new = rules::validate(input).map_err(Error::Validation)?;
    store::insert(conn, born(device, now), &new).await
}

/// A new scheme, host or port drops the pin (D-6).
///
/// # Errors
/// `Validation` (`url`, `label`); `NotFound` when there is no such interface.
pub async fn update(
    conn: &mut PgConnection,
    id: WebInterfaceId,
    input: &WebInterfaceInput,
) -> Result<Updated, Error> {
    let new = rules::validate(input).map_err(Error::Validation)?;
    let (before, after) = store::update(conn, id, &new).await?;
    Ok(Updated { before, after })
}

/// # Errors
/// `NotFound` when there is no such interface.
pub async fn delete(conn: &mut PgConnection, id: WebInterfaceId) -> Result<DeviceId, Error> {
    store::delete(conn, id).await
}

async fn http_check(db: &PgPool, id: CheckId) -> Result<(DeviceId, String), Error> {
    let check = checks::get(db, id).await.map_err(|error| match error {
        checks::Error::NotFound(checks::Missing::Check(id)) => Error::NotFound(Missing::Check(id)),
        other => Error::Checks(other),
    })?;
    match check.config {
        CheckConfig::Http(http) => Ok((check.device, http.url)),
        _ => Err(Error::NotFound(Missing::Check(id))),
    }
}

/// An interface already at that URL is returned and nothing is created (D-7).
///
/// # Errors
/// `NotFound` when the check does not exist or is not HTTP; `Validation` (`url`) for a bad URL.
pub async fn accept_suggestion(
    db: &PgPool,
    conn: &mut PgConnection,
    check: CheckId,
    now: DateTime<Utc>,
) -> Result<Accepted, Error> {
    let (device, url) = http_check(db, check).await?;
    let input = WebInterfaceInput {
        label: None,
        url,
        open_in: OpenIn::AnimaTab,
    };
    let new = rules::validate(&input).map_err(Error::Validation)?;
    // The device lock serialises accepts; it lasts to the end of the caller's transaction.
    let mut tx = conn.begin().await?;
    store::lock_device(&mut tx, device).await?;
    let existing = store::for_devices(&mut *tx, &[device]).await?;
    if let Some(interface) = existing
        .into_iter()
        .find(|interface| suggestion::same_url(&interface.url, &new.url))
    {
        return Ok(Accepted {
            interface,
            created: false,
        });
    }
    let interface = store::insert(&mut tx, born(device, now), &new).await?;
    tx.commit().await?;
    Ok(Accepted {
        interface,
        created: true,
    })
}

/// A second dismissal changes nothing; returns the check's device.
///
/// # Errors
/// `NotFound` when the check does not exist or is not an HTTP check.
pub async fn dismiss_suggestion(
    db: &PgPool,
    conn: &mut PgConnection,
    check: CheckId,
    now: DateTime<Utc>,
) -> Result<DeviceId, Error> {
    let (device, _) = http_check(db, check).await?;
    store::insert_dismissed(conn, check, now).await?;
    Ok(device)
}

/// Replaces an earlier pin (D-5); the row stays locked from the check to the pin.
///
/// # Errors
/// `NotFound` when there is no such interface; `Validation` (`fingerprintSha256`) for a bad fingerprint or `http`.
pub async fn accept_certificate(
    conn: &mut PgConnection,
    id: WebInterfaceId,
    acceptance: Acceptance<'_>,
) -> Result<WebInterface, Error> {
    let mut tx = conn.begin().await?;
    let interface = store::get_locked(&mut tx, id).await?;
    let fingerprint =
        rules::fingerprint(acceptance.fingerprint).map_err(|e| Error::Validation(vec![e]))?;
    if !rules::is_https(&interface.url) {
        return Err(Error::Validation(vec![FieldError::new(
            "fingerprintSha256",
            "Only an https interface has a certificate to accept.",
        )]));
    }
    let pin = Pin {
        fingerprint,
        at: acceptance.at,
    };
    store::set_pin(&mut tx, id, &pin, acceptance.operator).await?;
    tx.commit().await?;
    Ok(WebInterface {
        pin: Some(pin),
        ..interface
    })
}
