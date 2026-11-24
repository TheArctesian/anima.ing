//! The metrics GraphQL part (K1, K8 Section 3; CAP-030, API-003, MET-005, MET-012, MET-013,
//! R-364, D-9). Each mutation writes its change, one audit entry and one event in one
//! transaction; neither entry holds the code.

use async_graphql::futures_util::stream::{self, Stream};
use async_graphql::{Context, Error as GraphQlError, ID, Object, Result, Subscription};
use serde_json::json;
use tokio::sync::broadcast::{Receiver, error::RecvError};

use super::{
    CapabilityFilter, DaemonNeed, DeviceCapabilities, Feed, MetricsAdded,
    ingest::{self, DaemonEnrolment},
};
use crate::{
    api_errors::{
        ErrorCode, FieldError, begin, coded, commit, internal, invalid, pool, schema_dependency,
    },
    fleet::{
        access::{Authenticated, operator_only},
        devices::{self, Device},
        events::{AuditAction, AuditTarget, Severity, Source, Trail, TrailEntry},
    },
    ids::DeviceId,
};

#[derive(Default)]
pub struct Query;

#[Object(name = "MetricsQuery")]
impl Query {
    /// The tool inventory of each device whose daemon sent one, in device-name order; `tool`
    /// keeps the devices where it is usable, `buildTarget` those with that target (CAP-030).
    #[graphql(guard = "Authenticated")]
    async fn capabilities(
        &self,
        ctx: &Context<'_>,
        tool: Option<String>,
        build_target: Option<String>,
    ) -> Result<Vec<DeviceCapabilities>> {
        let db = pool(ctx)?;
        let filter = CapabilityFilter { tool, build_target };
        let found = super::capabilities(db, &filter).await.map_err(internal)?;
        let devices = devices::list(db).await.map_err(internal)?;
        Ok(super::capabilities::with_devices(found, devices))
    }
}

#[derive(Default)]
pub struct Mutation;

#[Object(name = "MetricsMutation")]
impl Mutation {
    /// A one-use code that enrols a daemon for the device within 15 minutes; the device's
    /// earlier unused codes stop working (MET-012). Login or browser token only (INT-003).
    #[graphql(guard = "Authenticated")]
    async fn enrol_daemon(&self, ctx: &Context<'_>, device_id: ID) -> Result<DaemonEnrolment> {
        let device = device_of(ctx, wanted(&device_id)?).await?;
        let trail = Trail::from_context(ctx)?;
        let mut tx = begin(ctx).await?;
        let enrolment = ingest::enrol_code(&mut tx, &device, trail)
            .await
            .map_err(ingest_error)?;
        commit(tx).await?;
        Ok(enrolment)
    }

    /// Ends the device's daemon token and its unused codes; the next push or renewal with the
    /// token is refused. The metrics source stays `DAEMON` (MET-013, R-092 D-7).
    #[graphql(guard = "Authenticated")]
    async fn revoke_daemon_token(&self, ctx: &Context<'_>, device_id: ID) -> Result<Device> {
        let device = device_of(ctx, wanted(&device_id)?).await?;
        let trail = Trail::from_context(ctx)?;
        let mut tx = begin(ctx).await?;
        let had_token = ingest::revoke(&mut tx, device.id)
            .await
            .map_err(ingest_error)?;
        let entry = token_revoked(&device, had_token);
        trail.write(&mut tx, entry).await.map_err(internal)?;
        commit(tx).await?;
        Ok(device)
    }

    /// Sets what the device's daemon waits for from a person at the device, or clears it with
    /// no need. The desktop app of that computer reports it, as the daemon has no token to tell
    /// the hub; the next accepted batch clears it. Login or browser token only (D-23,
    /// R-364).
    #[graphql(guard = "Authenticated")]
    async fn set_daemon_need(
        &self,
        ctx: &Context<'_>,
        device_id: ID,
        need: Option<DaemonNeed>,
    ) -> Result<Device> {
        operator_only(ctx, "setDaemonNeed")?;
        let device = device_of(ctx, wanted(&device_id)?).await?;
        let trail = Trail::from_context(ctx)?;
        let mut tx = begin(ctx).await?;
        let changed = ingest::set_need(&mut tx, device.id, need.map(DaemonNeed::stored), trail.now)
            .await
            .map_err(internal)?
            .ok_or_else(|| coded("This device has no daemon token.", ErrorCode::NotFound))?;
        if changed {
            trail
                .write(&mut tx, need_changed(&device, need))
                .await
                .map_err(internal)?;
        }
        commit(tx).await?;
        Ok(device)
    }
}

/// The device the mutation is about; `NOT_FOUND` when there is none.
async fn device_of(ctx: &Context<'_>, id: DeviceId) -> Result<Device> {
    devices::get(pool(ctx)?, id).await.map_err(device_error)
}

fn device_error(error: devices::Error) -> GraphQlError {
    match error {
        devices::Error::NotFound(_) => coded("No such device.", ErrorCode::NotFound),
        other => internal(other),
    }
}

/// `NOT_FOUND` for a device deleted after it was read; anything else is internal.
fn ingest_error(error: ingest::Error) -> GraphQlError {
    match error {
        ingest::Error::NoDevice => coded("No such device.", ErrorCode::NotFound),
        other => internal(other),
    }
}

/// The audit entry and event of a revoked token.
fn token_revoked(device: &Device, had_token: bool) -> TrailEntry {
    TrailEntry {
        source: Source::Metrics,
        severity: Severity::Info,
        device: Some((device.id, device.name.clone())),
        message: format!("Revoked the daemon token of {}.", device.name),
        action: AuditAction::Delete,
        target: AuditTarget::Device(device.id),
        change: json!({ "daemon": { "tokenRevoked": had_token } }),
    }
}

/// The audit entry and event of a changed need of the daemon (R-364).
fn need_changed(device: &Device, need: Option<DaemonNeed>) -> TrailEntry {
    let (message, stored) = match need {
        Some(DaemonNeed::Keychain) => (
            format!(
                "The daemon on {} waits for a person to allow it in the Keychain.",
                device.name
            ),
            Some(DaemonNeed::Keychain.stored()),
        ),
        None => (
            format!(
                "The daemon on {} no longer waits for a person.",
                device.name
            ),
            None,
        ),
    };
    TrailEntry {
        source: Source::Metrics,
        severity: Severity::Info,
        device: Some((device.id, device.name.clone())),
        message,
        action: AuditAction::Update,
        target: AuditTarget::Device(device.id),
        change: json!({ "daemon": { "need": stored } }),
    }
}

#[derive(Default)]
pub struct Subscription;

#[Subscription(name = "MetricsSubscription")]
impl Subscription {
    #[expect(
        clippy::unused_async,
        clippy::unused_async_trait_impl,
        reason = "the Subscription macro requires async resolvers"
    )]
    #[graphql(guard = "Authenticated")]
    async fn metrics_added(
        &self,
        ctx: &Context<'_>,
        device_id: Option<ID>,
    ) -> Result<impl Stream<Item = MetricsAdded>> {
        let device = device_id.as_ref().map(wanted).transpose()?;
        let feed = schema_dependency::<Feed>(ctx)?;
        Ok(added(feed.subscribe(), device))
    }
}

/// The device an id argument names.
fn wanted(id: &ID) -> Result<DeviceId> {
    id.parse()
        .map_err(|_| invalid(&[FieldError::new("deviceId", "That id is not valid.")]))
}

/// The collections of `device` (of any device when `None`). A subscriber that fell behind
/// skips what it missed; the stream ends when the feed is closed (D-9).
fn added(
    received: Receiver<MetricsAdded>,
    device: Option<DeviceId>,
) -> impl Stream<Item = MetricsAdded> {
    stream::unfold(received, move |mut received| async move {
        loop {
            match received.recv().await {
                Ok(added)
                    if device.is_none_or(|wanted| added.device_id.0 == wanted.to_string()) =>
                {
                    return Some((added, received));
                }
                Ok(_) | Err(RecvError::Lagged(_)) => {}
                Err(RecvError::Closed) => return None,
            }
        }
    })
}
