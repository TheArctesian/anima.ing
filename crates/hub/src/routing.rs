//! Compose the hub routes under the shared network and origin policy.
mod graphql;
pub mod web_client;

use std::{path::Path, sync::Arc};

use axum::{Router, middleware};
use sqlx::PgPool;

pub use graphql::{BODY_LIMIT, router, router_at};

#[cfg(unix)]
use crate::work;
use crate::{clock, engine, fleet, fleet::access::CLIENT_ORIGINS, schema};

/// The routes devices push to, logged by their templates only, never their tokens (LOG-007,
/// R-028): `/push/{token}` for push checks, and the daemon's enrolment, metrics batches and
/// operational key fetches (K8, K-E5 Section 2.1).
pub fn pushes(dependencies: &schema::Dependencies) -> Router {
    let schema::Dependencies {
        clock,
        pool,
        bus,
        metrics_feed,
        vault,
        sources,
        ..
    } = dependencies;
    let daemon = fleet::metrics::daemon_router(
        (pool.clone(), bus.clone(), clock.clone()),
        metrics_feed.clone(),
        vault.clone(),
    )
    .layer(axum::Extension(
        fleet::compute::policy::model_budget as crate::work::jobs::ModelPolicy,
    ))
    .layer(axum::Extension(crate::work::checkout::Issuer(
        engine::sources::Scheduling::new(
            (pool.clone(), bus.clone(), clock.clone()),
            vault.clone(),
            sources.clone(),
        ),
    )));
    logged(fleet::checks::push_router(pool.clone(), bus.clone(), clock.clone()).merge(daemon))
}

/// Every route the hub serves, as `main.rs` serves it: `/graphql` and `/health` at the public
/// origin of `dependencies.config`, then the routes [`beside_graphql`] adds, all behind the
/// permitted-networks layer of `edge` (K3, NET-002). The schema takes `dependencies`. In the
/// link mode a request must also name a loopback host and come from no origin or a client
/// origin ([`fleet::access::loopback_names_layer`], FRN-007).
pub fn all_routes(dependencies: schema::Dependencies, edge: fleet::access::Edge) -> Router {
    let devices = pushes(&dependencies);
    all_routes_with_devices(dependencies, edge, devices)
}

/// Enables the private artifact routes using the store opened by hub startup.
#[cfg(unix)]
pub fn all_routes_with_artifacts(
    dependencies: schema::Dependencies,
    edge: fleet::access::Edge,
    store: Option<work::artifacts::Store>,
) -> Router {
    let devices = pushes(&dependencies);
    let devices = match store {
        Some(store) => {
            let downloads = work::artifacts::download_router(
                dependencies.pool.clone(),
                dependencies.clock.clone(),
                store.clone(),
            );
            devices
                .layer(axum::Extension(store))
                .merge(logged(downloads))
                .merge(logged(fleet::agent_threads::handoffs::operator::router(
                    dependencies.pool.clone(),
                    dependencies.clock.clone(),
                )))
        }
        None => devices,
    };
    all_routes_with_devices(dependencies, edge, devices)
}

fn all_routes_with_devices(
    dependencies: schema::Dependencies,
    edge: fleet::access::Edge,
    devices: Router,
) -> Router {
    let origin = fleet::access::PublicOrigin::new(dependencies.config.public_origin.clone());
    let link_mode = dependencies.config.link_socket.is_some();
    // Only a hosted hub is reached through the gateway of the pool, by the public origin that the
    // pool gave it (HMD-061, HMD-069).
    let gateway = dependencies.config.gateway_given().then(|| origin.clone());
    let preview =
        dependencies
            .config
            .preview_origins
            .clone()
            .map(|origins| fleet::code_previews::Browser {
                pool: dependencies.pool.clone(),
                clock: dependencies.clock.clone(),
                origins,
                app_origin: dependencies.config.public_origin.clone(),
                policy: dependencies.policy.clone(),
                slots: Arc::new(tokio::sync::Semaphore::new(128)),
            });
    let beside = beside_graphql(&dependencies, &origin, devices);
    let (pool, clock) = (dependencies.pool.clone(), dependencies.clock.clone());
    let routes = router_at(schema::build_schema(dependencies), pool, clock, origin).merge(beside);
    let routes = if link_mode {
        routes.layer(fleet::access::loopback_names_layer(
            &CLIENT_ORIGINS,
            gateway,
        ))
    } else {
        routes
    };
    let routes = if let Some(preview) = preview {
        routes.layer(axum::middleware::from_fn_with_state(
            preview,
            fleet::code_previews::route,
        ))
    } else {
        routes
    };
    routes.layer(fleet::access::permitted_networks_layer(edge))
}

/// The routes beside `/graphql`, each logged by its template only, never its token (K3,
/// LOG-007, R-028, R-051): the session tunnels, the push routes ([`pushes`]), `/metrics` behind a READ bearer
/// (D-24), Connect GitHub ([`connect_github`]), the engine's routes ([`schema::engine_routes`]) and, last, the web client at `config.web_root` as the fallback (D-2).
fn beside_graphql(
    dependencies: &schema::Dependencies,
    origin: &fleet::access::PublicOrigin,
    devices: Router,
) -> Router {
    let (pool, clock) = (&dependencies.pool, &dependencies.clock);
    let web_root = dependencies.config.web_root.as_deref();
    tunnels(dependencies)
        .merge(devices)
        .merge(exposition(pool.clone(), clock.clone()))
        .merge(connect_github(dependencies, origin))
        .merge(schema::engine_routes(dependencies))
        .merge(web_fallback(web_root, origin))
}

/// Runs every route of `routes` in an `http_request` span that records the route template,
/// never a path value, so `/push/{token}` is logged as `/push/[redacted]` (LOG-007, R-028).
/// Apply it to a router after its last route is added; [`all_routes`] wraps each router it
/// merges in, such as `/tunnel` and `/push`.
pub fn logged(routes: Router) -> Router {
    routes.layer(middleware::from_fn(
        fleet::events::process_log::http_request,
    ))
}

/// The web client as the hub's fallback when `root` is set (D-2): any other `GET` gets a file
/// or `index.html`, in a logged span like every route. Merge it beside the explicit routes,
/// which win, and before the NET-002 layer, which then covers it. Without a root it adds
/// nothing, so an unknown path gets 404.
pub fn web_fallback(root: Option<&Path>, origin: &fleet::access::PublicOrigin) -> Router {
    root.map_or_else(Router::new, |root| {
        logged(web_client::web_client(root, origin))
    })
}

/// The Connect GitHub routes (`/connect/github*`, K-E1 Section 7a), logged by their templates
/// only: a ticket, a code or a state in a query never reaches the log (LOG-007).
pub fn connect_github(
    dependencies: &schema::Dependencies,
    origin: &fleet::access::PublicOrigin,
) -> Router {
    let hub = (
        dependencies.pool.clone(),
        dependencies.bus.clone(),
        dependencies.clock.clone(),
    );
    let reach = dependencies.sources.clone();
    let flow = engine::sources::GithubFlow {
        attempts: reach.attempts.clone(),
        scheduling: engine::sources::Scheduling::new(hub, dependencies.vault.clone(), reach),
        origin: origin.clone(),
    };
    logged(engine::sources::github_connect_router(flow))
}

/// `GET /metrics` for scrapers (INT-001, D-24): the checks' exposition behind a bearer token of
/// READ scope or more, in a logged span (K3, WP-43 A-9). A refused scrape gets 401 and starts
/// no build. The layer wraps the route only, never the fallback, so a merge cannot put the
/// bearer check on unknown paths. Merge it before the NET-002 layer, which then covers it.
pub fn exposition(db: PgPool, clock: Arc<dyn clock::Clock>) -> Router {
    let scrapes = fleet::checks::exposition_router(db.clone(), clock.clone());
    logged(scrapes.route_layer(fleet::access::bearer_layer(
        db,
        clock,
        fleet::access::Scope::Read,
    )))
}

/// The session tunnels, each logged by its template only (K4 Sections 8, 9): `/terminal` for
/// SSH, `/tunnel` to guacd for RDP and VNC, which refuses SSH tickets so none reaches guacd
/// (R-062), and `/files`, which connects through the link and so gets the probes' deny list.
///
/// `main.rs` builds one address policy for every outbound connection (R-030, R-032): `/files`,
/// the probes and the collector take it as `policy`; the terminal, the Guacamole tunnel and the
/// connection tests take it through `live`'s target rule ([`fleet::sessions::Live::denying`]).
pub fn tunnels(dependencies: &schema::Dependencies) -> Router {
    let schema::Dependencies {
        config,
        clock,
        pool,
        bus,
        vault,
        live,
        policy,
        ..
    } = dependencies;
    let serving = fleet::sessions::Serving {
        db: pool.clone(),
        vault: vault.clone(),
        bus: bus.clone(),
        clock: clock.clone(),
        live: live.clone(),
        guacd: config.guacd.clone(),
        drive_root: config.guacd_drive_root.to_string_lossy().into_owned(),
    };
    let terminal = fleet::sessions::terminal_router(&serving);
    let tunnel = fleet::sessions::tunnel_router(&serving);
    let files = fleet::sessions::files_router(
        pool.clone(),
        vault.clone(),
        bus.clone(),
        clock.clone(),
        live.clone(),
        policy.clone(),
    );
    logged(terminal).merge(logged(tunnel)).merge(logged(files))
}
