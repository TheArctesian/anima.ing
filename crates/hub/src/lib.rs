// The merged schema roots nest one future per merged object, past the default limit of 128
// (OD-E14).
#![recursion_limit = "256"]

pub mod api_errors;
pub mod clock;
pub mod config;
pub mod engine;
pub mod fingerprint;
pub mod fleet;
pub mod ids;
pub mod paging;
pub mod scalars;
pub mod schema;
pub mod status;
pub mod text;
pub mod work;

mod routing;
pub use fleet::access::CLIENT_ORIGINS;
#[cfg(unix)]
pub use routing::all_routes_with_artifacts;
pub use routing::{
    BODY_LIMIT, all_routes, connect_github, exposition, logged, pushes, router, router_at, tunnels,
    web_client, web_fallback,
};

use std::{future::Future, sync::Arc, time::Duration};

use axum::Router;
use sqlx::PgPool;

/// The sources the hub reads from, and the declared API bases its transport may reach: the
/// only outbound destinations of the sources engine. `intakes` are the extensions' (K-E1
/// Section 3); every address still passes `policy` (R-030).
///
/// # Errors
/// [`engine::sources::BuildFailed`] when the TLS client cannot be built.
pub fn source_reach(
    intakes: Vec<Arc<dyn engine::sources::Intake>>,
    policy: fleet::targets::AddressPolicy,
) -> Result<engine::sources::Reach, engine::sources::BuildFailed> {
    let declarations = vec![provider_github::DECLARATION, provider_dawarich::DECLARATION];
    let bases: Vec<&str> = declarations.iter().map(|d| d.api_base).collect();
    let transport = engine::sources::Transport::new(bases, policy)?;
    let registry = engine::sources::Registry::new(declarations, intakes);
    Ok(engine::sources::Reach::new(registry, transport))
}

/// The longest the drain may wait for open requests after the stop signal. A client can hold
/// a request open for ever (a partial body, say); past this the hub stops anyway, and its
/// sessions have already ended (R-040).
pub const DRAIN_LIMIT: Duration = Duration::from_secs(10);

/// What the hub ends when it stops, and how long it waits for open requests (R-040).
pub struct Shutdown {
    pub db: PgPool,
    /// The `Live` the schema and the tunnel serve with.
    pub live: fleet::sessions::Live,
    pub clock: Arc<dyn clock::Clock>,
    /// [`DRAIN_LIMIT`] in the hub.
    pub drain_limit: Duration,
}

/// Serves `app` through `entrance` (the NET-002 accept-time check, TLS when configured), with
/// each request's TCP peer as its connect info, until `signal` resolves. Then, in order: ends
/// every session `shutdown.live` holds with `HUB_SHUTDOWN`, because the drain does not end
/// upgraded connections such as tunnels; then stops accepting and drains the open requests for
/// at most `shutdown.drain_limit` (R-040).
pub async fn serve_until(
    entrance: fleet::access::listen::Entrance,
    app: Router,
    shutdown: Shutdown,
    signal: impl Future<Output = ()> + Send + 'static,
) {
    let Shutdown {
        db,
        live,
        clock,
        drain_limit,
    } = shutdown;
    let stop = async move {
        signal.await;
        if let Err(error) = fleet::sessions::shut_down(&db, &live, clock.now()).await {
            // The error's text is fixed and holds no value (fleet::sessions::Error, LOG-007).
            tracing::error!(%error, "sessions did not all end at shutdown");
        }
    };
    entrance.serve_until(app, stop, drain_limit).await;
}
