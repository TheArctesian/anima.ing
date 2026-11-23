//! The headers on every web client response (K7 "Static file headers").

use axum::http::{HeaderName, HeaderValue, StatusCode, header};

use super::path::Wanted;

/// The fallback when a page's own policy cannot be a header: scripts are then blocked, never let through.
const STRICTEST: &str = "default-src 'self'; script-src 'self'; connect-src 'self'; \
                         img-src 'self' data: blob:; style-src 'self' 'unsafe-inline'; \
                         font-src 'self'; object-src 'none'; base-uri 'none'; \
                         form-action 'none'; frame-ancestors 'none'";

const FIXED: [(HeaderName, &str); 4] = [
    (header::X_CONTENT_TYPE_OPTIONS, "nosniff"),
    (header::REFERRER_POLICY, "no-referrer"),
    (header::STRICT_TRANSPORT_SECURITY, "max-age=31536000"),
    (
        HeaderName::from_static("cross-origin-opener-policy"),
        "same-origin",
    ),
];

const NO_CACHE: &str = "no-cache";
const IMMUTABLE: &str = "public, max-age=31536000, immutable";

/// A `websocket` that is not plain origin text is left out, so it can never add a directive (A-2).
#[must_use]
pub fn content_security_policy(script_hashes: &[String], websocket: &str) -> HeaderValue {
    let scripts = script_hashes.iter().fold(String::new(), |mut text, hash| {
        text.push(' ');
        text.push_str(hash);
        text
    });
    let connect = if is_origin_text(websocket) {
        format!(" {websocket}")
    } else {
        String::new()
    };
    let policy = format!(
        "default-src 'self'; script-src 'self'{scripts}; connect-src 'self'{connect}; \
         img-src 'self' data: blob:; style-src 'self' 'unsafe-inline'; font-src 'self'; \
         object-src 'none'; base-uri 'none'; form-action 'none'; frame-ancestors 'none'"
    );
    HeaderValue::from_str(&policy).unwrap_or(HeaderValue::from_static(STRICTEST))
}

fn is_origin_text(text: &str) -> bool {
    !text.is_empty()
        && text
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b":/.-[]".contains(&byte))
}

/// Everything but an immutable asset gets `no-cache`, refusals included, so no browser keeps a stale or missing answer.
#[must_use]
pub fn cache_control(wanted: Wanted, status: StatusCode) -> HeaderValue {
    let served = status.is_success() || status == StatusCode::NOT_MODIFIED;
    if served && wanted == (Wanted::Asset { immutable: true }) {
        HeaderValue::from_static(IMMUTABLE)
    } else {
        HeaderValue::from_static(NO_CACHE)
    }
}

pub fn apply(headers: &mut axum::http::HeaderMap, policy: &HeaderValue, cache: HeaderValue) {
    for (name, value) in FIXED {
        headers.insert(name, HeaderValue::from_static(value));
    }
    headers.insert(header::CONTENT_SECURITY_POLICY, policy.clone());
    headers.insert(header::CACHE_CONTROL, cache);
}
