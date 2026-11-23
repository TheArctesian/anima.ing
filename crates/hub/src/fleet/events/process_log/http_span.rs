//! One span and one access line per HTTP request, naming the route and never a value in the
//! path (LOG-002, LOG-007, R-028).

use axum::{
    extract::{MatchedPath, Request},
    http::Method,
    middleware::Next,
    response::Response,
};
use tracing::{Instrument as _, info_span};

/// An unmatched request's path is the sender's text, so it is not logged.
const NO_ROUTE: &str = "[no route]";
const REDACTED: &str = "[redacted]";
const OTHER_METHOD: &str = "[other]";

/// Every route parameter is redacted, so the `/push/{token}` token (R-028) is never logged.
/// Add it with `Router::layer` after every route, so each route's template is known.
pub async fn http_request(request: Request, next: Next) -> Response {
    let path = logged_path(
        request
            .extensions()
            .get::<MatchedPath>()
            .map(MatchedPath::as_str),
    );
    let method = logged_method(request.method());
    let span = info_span!("http_request", method, path);
    async move {
        let response = next.run(request).await;
        tracing::info!(status = response.status().as_u16(), "request served");
        response
    }
    .instrument(span)
    .await
}

/// A non-standard method is the sender's text, so it is logged as `[other]` (LOG-007).
pub(super) fn logged_method(method: &Method) -> &'static str {
    const STANDARD: [&str; 9] = [
        "GET", "POST", "PUT", "DELETE", "HEAD", "OPTIONS", "PATCH", "CONNECT", "TRACE",
    ];
    STANDARD
        .into_iter()
        .find(|name| *name == method.as_str())
        .unwrap_or(OTHER_METHOD)
}

pub(super) fn logged_path(template: Option<&str>) -> String {
    let Some(template) = template else {
        return NO_ROUTE.to_owned();
    };
    template
        .split('/')
        .map(|segment| {
            if segment.starts_with('{') {
                REDACTED
            } else {
                segment
            }
        })
        .collect::<Vec<_>>()
        .join("/")
}
