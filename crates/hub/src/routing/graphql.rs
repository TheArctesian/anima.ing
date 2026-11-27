//! HTTP and WebSocket entry points for the GraphQL schema.

use std::{sync::Arc, time::Duration};

use async_graphql_axum::GraphQLResponse;
use axum::{
    Router,
    body::Bytes,
    extract::{
        DefaultBodyLimit, State, WebSocketUpgrade, ws::rejection::WebSocketUpgradeRejection,
    },
    http::{
        Extensions, HeaderMap, HeaderValue, Method, StatusCode,
        header::{ACCEPT, AUTHORIZATION, CONTENT_TYPE},
    },
    response::{IntoResponse, Response},
    routing::{get, post},
};
use sqlx::PgPool;
use tower_http::cors::{AllowOrigin, CorsLayer};

use super::logged;
use crate::{clock, fleet, fleet::access::CLIENT_ORIGINS, schema};

/// Largest `/graphql` request body. A bigger one gets 413 before it is parsed or any token
/// or password is checked (SEC-002).
pub const BODY_LIMIT: usize = 1024 * 1024;

/// What the `/graphql` handler needs besides the request.
#[derive(Clone)]
struct GraphqlRoute {
    schema: schema::HubSchema,
    db: PgPool,
    clock: Arc<dyn clock::Clock>,
    /// `ANIMA_PUBLIC_ORIGIN`; without it no request is marked same-origin, so no browser
    /// cookie is read and `signInBrowser` is refused (D-3, D-4).
    origin: Option<fleet::access::PublicOrigin>,
}

/// How long a web view may reuse a preflight answer.
const PREFLIGHT_MAX_AGE: Duration = Duration::from_secs(600);

/// The CORS rule for `/graphql`: the client origins may `POST` with the headers the client sends.
fn client_origins() -> CorsLayer {
    CorsLayer::new()
        .allow_origin(AllowOrigin::list(
            CLIENT_ORIGINS.map(HeaderValue::from_static),
        ))
        .allow_methods([Method::POST])
        .allow_headers([CONTENT_TYPE, ACCEPT, AUTHORIZATION])
        .max_age(PREFLIGHT_MAX_AGE)
}

/// The hub's HTTP routes: `POST /graphql`, `GET /graphql` (subscriptions) and `GET /health`,
/// each request in a logged span. `/graphql` answers CORS preflights from the client origins.
/// Without a public origin it reads no browser cookie; [`router_at`] serves browsers too.
pub fn router(schema: schema::HubSchema, db: PgPool, clock: Arc<dyn clock::Clock>) -> Router {
    routes(GraphqlRoute {
        schema,
        db,
        clock,
        origin: None,
    })
}

/// The routes of [`router`] for clients at `origin`, `ANIMA_PUBLIC_ORIGIN`: a request from that
/// origin may also sign in with the browser cookie and call `signInBrowser` (D-3, D-4).
pub fn router_at(
    schema: schema::HubSchema,
    db: PgPool,
    clock: Arc<dyn clock::Clock>,
    origin: fleet::access::PublicOrigin,
) -> Router {
    routes(GraphqlRoute {
        schema,
        db,
        clock,
        origin: Some(origin),
    })
}

fn routes(route: GraphqlRoute) -> Router {
    logged(
        Router::new()
            .route("/graphql", post(graphql).get(subscriptions))
            .route("/health", get(health))
            .layer(DefaultBodyLimit::max(BODY_LIMIT))
            .layer(client_origins())
            .with_state(route),
    )
}

/// Liveness probe (R-026(4)): no sign-in, no database, body `ok`.
async fn health() -> &'static str {
    "ok"
}

/// Whether the media type is exactly `application/json`, in any case, parameters allowed.
fn is_json(headers: &HeaderMap) -> bool {
    headers
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split(';').next())
        .is_some_and(|media| media.trim().eq_ignore_ascii_case("application/json"))
}

/// Reads one JSON request of at most [`BODY_LIMIT`] bytes (no batch, no multipart), resolves
/// the client address, the Origin mark and the bearer token or browser cookie once, then runs
/// it (D-3, D-6).
async fn graphql(
    State(route): State<GraphqlRoute>,
    headers: HeaderMap,
    extensions: Extensions,
    body: Bytes,
) -> Response {
    if !is_json(&headers) {
        return (StatusCode::UNSUPPORTED_MEDIA_TYPE, "Send one JSON request.").into_response();
    }
    let Ok(mut request) = serde_json::from_slice::<async_graphql::Request>(&body) else {
        return (
            StatusCode::BAD_REQUEST,
            "The request is not a GraphQL request.",
        )
            .into_response();
    };
    if let Some(client) = fleet::access::client_addr(&extensions) {
        request = request.data(client);
    }
    // For a sign-in, which has no identity yet to carry it (LOG-016).
    request = request.data(fleet::access::Declaration::of_headers(&headers));
    let from =
        (route.origin.as_ref()).and_then(|origin| fleet::access::SameOrigin::of(&headers, origin));
    let identified =
        fleet::access::identify(&route.db, &headers, from.as_ref(), route.clock.now()).await;
    if let Some(from) = from {
        request = request.data(from);
    }
    let response = match identified {
        Ok(Some(identity)) => route.schema.execute(identity.attach(request)).await,
        Ok(None) => route.schema.execute(request).await,
        // The schema's boundary never sees this error, so the route logs its cause chain (H-5).
        Err(error) => {
            let error = error.into_response_error();
            fleet::events::process_log::log_failure(&error);
            async_graphql::Response::from_errors(vec![error])
        }
    };
    GraphQLResponse::from(response).into_response()
}

/// Upgrades `GET /graphql` to a `graphql-transport-ws` connection. The token comes from the
/// `connection_init` payload, or from the browser cookie of an upgrade from the public origin;
/// never from the URL (R-020, D-3). A `GET` that is not a WebSocket upgrade gets 405: queries
/// are never read from a URL.
async fn subscriptions(
    State(route): State<GraphqlRoute>,
    headers: HeaderMap,
    extensions: Extensions,
    upgrade: Result<WebSocketUpgrade, WebSocketUpgradeRejection>,
) -> Response {
    let Ok(upgrade) = upgrade else {
        return StatusCode::METHOD_NOT_ALLOWED.into_response();
    };
    let mut access = fleet::access::SocketAccess::new(
        route.db,
        route.clock,
        fleet::access::client_addr(&extensions),
    );
    if let Some(origin) = &route.origin {
        access = access.with_upgrade(&headers, origin);
    }
    fleet::access::accept_socket(upgrade, route.schema, access)
}
