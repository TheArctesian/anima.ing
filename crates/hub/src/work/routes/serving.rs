//! What serves now: the one view of the routes that the proxy gets through [`super::door`]
//! (Section 12.3, SF-231). A route serves only when it waits for nothing
//! ([`Route::waits`]).

use std::net::SocketAddr;

use chrono::{DateTime, Utc};
use sqlx::PgPool;

use super::{Error, Route, certificates, kinds, names::RouteName, store};
use crate::{
    fleet::credentials::{self, Secret, Vault},
    ids::{ApplicationId, DeviceId},
};

/// The certificate of a serving route with its key opened, for the proxy only (through
/// [`super::door`]). No `Debug`: it holds the key.
pub struct Served {
    pub name: RouteName,
    pub access: kinds::Access,
    /// The one address of a redirect; `None` for a web route.
    pub redirect_to: Option<String>,
    pub application: ApplicationId,
    pub chain_pem: String,
    pub key_pem: Secret,
}

/// Each route that serves at `now`, with its certificate and key: the one view of the routes
/// that the proxy gets (Section 12.3).
///
/// # Errors
/// `Database`, `Unreadable` or `Seal`.
pub async fn served(db: &PgPool, vault: &Vault, now: DateTime<Utc>) -> Result<Vec<Served>, Error> {
    let mut found = Vec::new();
    for route in store::list(db, None).await? {
        if route.kind.is_stream() || route.waits(now).is_some() {
            continue;
        }
        let Some(stored) = store::certificate(db, route.id).await? else {
            continue;
        };
        let sealed = credentials::Sealed::from_bytes(&stored.sealed_key).map_err(Error::Seal)?;
        let context = certificates::key_context(route.id, &route.name);
        let key_pem = vault
            .open(&sealed, context.as_bytes())
            .map_err(Error::Seal)?;
        found.push(Served {
            name: route.name,
            access: route.access,
            redirect_to: route.redirect_to,
            application: route.application,
            chain_pem: stored.chain_pem,
            key_pem,
        });
    }
    Ok(found)
}

/// Each stream that serves at `now`: the ports of the hub that the proxy listens on.
///
/// # Errors
/// `Database` or `Unreadable`.
pub async fn served_streams(db: &PgPool, now: DateTime<Utc>) -> Result<Vec<Route>, Error> {
    Ok(store::list(db, None)
        .await?
        .into_iter()
        .filter(|route| route.kind.is_stream() && route.waits(now).is_none())
        .collect())
}

/// The fixed target of the route `name` of the application, when it serves at `now`, with its
/// kind: the one way from an application identifier to a device and a port
/// (SF-231). A redirect has no target.
///
/// # Errors
/// `NotFound` when no such route of the application serves; `Database` or `Unreadable`.
pub async fn target_of(
    db: &PgPool,
    application: ApplicationId,
    name: &str,
    now: DateTime<Utc>,
) -> Result<(DeviceId, SocketAddr, kinds::Kind), Error> {
    store::list(db, Some(application))
        .await?
        .into_iter()
        .filter(|route| route.name.as_str() == name && route.waits(now).is_none())
        .find_map(|route| {
            route
                .address
                .map(|address| (route.device, address, route.kind))
        })
        .ok_or(Error::NotFound)
}
