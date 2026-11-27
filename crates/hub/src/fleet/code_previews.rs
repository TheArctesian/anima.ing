//! Preview hosts terminate their own browser sessions before the ordinary app router.
mod bootstrap;
mod forward;
mod headers;
mod proxy;
use super::access::CLIENT_ORIGINS;
use super::agent_threads::managed::browser_sessions::Audience;
use crate::{clock::Clock, config::PreviewOrigins};
use agent_threads::managed::preview::FORWARD_PATH;
use axum::{
    body::to_bytes,
    extract::{Request, State},
    http::{HeaderValue, Method, StatusCode, header},
    middleware::Next,
    response::{IntoResponse, Response},
};
use sqlx::PgPool;
use std::sync::Arc;

pub(crate) const COOKIE: &str = "__Host-anima-preview";
#[derive(Clone)]
pub(crate) struct Browser {
    pub pool: PgPool,
    pub clock: Arc<dyn Clock>,
    pub origins: PreviewOrigins,
    pub app_origin: String,
    pub policy: super::targets::AddressPolicy,
    pub slots: Arc<tokio::sync::Semaphore>,
}

pub(crate) async fn route(State(state): State<Browser>, request: Request, next: Next) -> Response {
    let mut hosts = request.headers().get_all(header::HOST).iter();
    let Some(host) = hosts.next().and_then(|value| value.to_str().ok()) else {
        return next.run(request).await;
    };
    if hosts.next().is_some() {
        return refusal(StatusCode::BAD_REQUEST);
    }
    let Some(endpoint) = state.origins.endpoint(host) else {
        if state.origins.contains(host) {
            return refusal(StatusCode::NOT_FOUND);
        }
        return next.run(request).await;
    };
    if request.uri().path() == "/.anima/connect" {
        return if request.method() == Method::GET {
            bootstrap::page(&state)
        } else {
            refusal(StatusCode::METHOD_NOT_ALLOWED)
        };
    }
    if request.uri().path() == "/.anima/handoff" {
        return handoff(&state, endpoint, request).await;
    }
    if request.uri().path() == FORWARD_PATH {
        return forward::serve(state, endpoint, request).await;
    }
    proxy::serve(state, endpoint, request).await
}

async fn handoff(state: &Browser, endpoint: uuid::Uuid, request: Request) -> Response {
    if request.method() != Method::POST || request.headers().contains_key(header::AUTHORIZATION) {
        return refusal(StatusCode::METHOD_NOT_ALLOWED);
    }
    let mut origins = request.headers().get_all(header::ORIGIN).iter();
    let origin = origins.next().and_then(|value| value.to_str().ok());
    let allowed = origin.is_some_and(|origin| {
        origin == state.app_origin
            || origin == state.origins.origin(endpoint)
            || CLIENT_ORIGINS.contains(&origin)
    });
    if !allowed || origins.next().is_some() {
        return refusal(StatusCode::FORBIDDEN);
    }
    if request
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .is_none_or(|value| value.split(';').next() != Some("application/x-www-form-urlencoded"))
    {
        return refusal(StatusCode::UNSUPPORTED_MEDIA_TYPE);
    }
    let Ok(bytes) = to_bytes(request.into_body(), 256).await else {
        return refusal(StatusCode::BAD_REQUEST);
    };
    let Some(token) = std::str::from_utf8(&bytes)
        .ok()
        .and_then(|text| text.strip_prefix("token="))
    else {
        return refusal(StatusCode::BAD_REQUEST);
    };
    let Ok((cookie, lifetime)) = Audience::Browser
        .redeem(&state.pool, (endpoint, token), state.clock.now())
        .await
    else {
        return refusal(StatusCode::UNAUTHORIZED);
    };
    // A same-origin document initiates the navigation; a cross-site 303 can withhold
    // the newly issued SameSite=Strict cookie on its redirected request.
    let mut response = axum::response::Html("<!doctype html><meta charset=utf-8><title>Opening preview</title><script>location.replace('/')</script><noscript><a href=/>Open preview</a></noscript>").into_response();
    let Ok(cookie) = HeaderValue::from_str(&format!(
        "{COOKIE}={cookie}; Path=/; Secure; HttpOnly; SameSite=Strict; Max-Age={lifetime}"
    )) else {
        return refusal(StatusCode::INTERNAL_SERVER_ERROR);
    };
    response.headers_mut().insert(header::SET_COOKIE, cookie);
    response.headers_mut().insert(header::CONTENT_SECURITY_POLICY, HeaderValue::from_static("default-src 'none'; script-src 'sha256-qBjLReXPsRLHPao6+8nICoVlbEWZcc/Os2pcXx5BCa0='; base-uri 'none'; form-action 'none'; frame-ancestors 'none'"));
    protect(&mut response);
    response
}
fn refusal(status: StatusCode) -> Response {
    let mut response = status.into_response();
    protect(&mut response);
    response
}
fn protect(response: &mut Response) {
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response.headers_mut().insert(
        header::REFERRER_POLICY,
        HeaderValue::from_static("no-referrer"),
    );
    response.headers_mut().insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
}
