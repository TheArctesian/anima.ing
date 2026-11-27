//! One HTTP request per leased tunnel, with revocation watching for bodies and upgrades.
use super::{Browser, COOKIE, headers, refusal};
use crate::fleet::{
    access,
    agent_threads::managed::{browser_sessions::Audience, previews},
    devices, targets,
};
use agent_threads::managed::preview::Binding;
use axum::{
    body::Body,
    extract::Request,
    http::{HeaderMap, Method, StatusCode, Uri, header},
    response::Response,
};
use hyper_util::rt::TokioIo;
use std::{net::Ipv6Addr, sync::Arc, time::Duration};
use tokio::sync::OwnedSemaphorePermit;
use uuid::Uuid;

pub(super) async fn serve(state: Browser, endpoint: Uuid, mut request: Request) -> Response {
    let Ok(slot) = Arc::clone(&state.slots).try_acquire_owned() else {
        return refusal(StatusCode::TOO_MANY_REQUESTS);
    };
    let slot = Arc::new(slot);
    let origin = state.origins.origin(endpoint);
    if !same_origin(&request, &origin)
        || request.method() == Method::CONNECT
        || request.method() == Method::TRACE
        || request.headers().contains_key("service-worker")
        || request.uri().path().starts_with("/.anima/")
    {
        return refusal(StatusCode::FORBIDDEN);
    }
    let (binding, stream) = match connect(&state, endpoint, request.headers()).await {
        Ok(connected) => connected,
        Err(status) => return refusal(status),
    };
    let client_upgrade = match prepare(&mut request, binding.port) {
        Ok(upgrade) => upgrade,
        Err(status) => return refusal(status),
    };
    let Ok((mut sender, connection)) =
        hyper::client::conn::http1::handshake::<_, Body>(TokioIo::new(stream)).await
    else {
        return refusal(StatusCode::BAD_GATEWAY);
    };
    let watching = state.clone();
    let expected = binding.clone();
    let driver_slot = Arc::clone(&slot);
    let driver = tokio::spawn(async move {
        let _slot = driver_slot;
        tokio::select! {
            _ = connection.with_upgrades() => (),
            () = revoked(&watching, &expected) => (),
        }
    });
    let response =
        tokio::time::timeout(Duration::from_secs(30), sender.send_request(request)).await;
    let Ok(Ok(mut response)) = response else {
        driver.abort();
        return refusal(StatusCode::BAD_GATEWAY);
    };
    if response.status() == StatusCode::SWITCHING_PROTOCOLS {
        let Some((client, upstream)) = upgrades(client_upgrade, &mut response) else {
            driver.abort();
            return refusal(StatusCode::BAD_GATEWAY);
        };
        let relay = Relay {
            state,
            binding: binding.clone(),
            slot,
        };
        tokio::spawn(relay.bridge(client, upstream));
        headers::response(response.headers_mut(), binding.port, &origin, true);
    } else {
        headers::response(response.headers_mut(), binding.port, &origin, false);
    }
    response.map(Body::new)
}

fn upgrades(
    client: Option<hyper::upgrade::OnUpgrade>,
    response: &mut hyper::Response<hyper::body::Incoming>,
) -> Option<(hyper::upgrade::OnUpgrade, hyper::upgrade::OnUpgrade)> {
    let client = client?;
    if !response
        .headers()
        .get(header::UPGRADE)
        .is_some_and(|value| value.as_bytes().eq_ignore_ascii_case(b"websocket"))
    {
        return None;
    }
    Some((client, hyper::upgrade::on(response)))
}

fn prepare(
    request: &mut Request,
    port: u16,
) -> Result<Option<hyper::upgrade::OnUpgrade>, StatusCode> {
    let websocket = request
        .headers()
        .get(header::UPGRADE)
        .is_some_and(|value| value.as_bytes().eq_ignore_ascii_case(b"websocket"));
    if request.headers().contains_key(header::UPGRADE) && !websocket {
        return Err(StatusCode::BAD_REQUEST);
    }
    if websocket && request.method() != Method::GET {
        return Err(StatusCode::BAD_REQUEST);
    }
    let client_upgrade = websocket.then(|| hyper::upgrade::on(&mut *request));
    let path = request
        .uri()
        .path_and_query()
        .map_or("/", |value| value.as_str());
    let Ok(uri) = path.parse::<Uri>() else {
        return Err(StatusCode::BAD_REQUEST);
    };
    *request.uri_mut() = uri;
    headers::request(request.headers_mut(), port, websocket);
    Ok(client_upgrade)
}

async fn connect(
    state: &Browser,
    endpoint: Uuid,
    headers: &HeaderMap,
) -> Result<(Binding, targets::Reached), StatusCode> {
    let cookie = access::cookie_value(headers, COOKIE).ok_or(StatusCode::UNAUTHORIZED)?;
    let principal = Audience::Browser
        .principal(&state.pool, (endpoint, cookie), state.clock.now())
        .await
        .map_err(|_| StatusCode::UNAUTHORIZED)?;
    connect_grant(state, endpoint, principal).await
}

pub(super) async fn connect_grant(
    state: &Browser,
    endpoint: Uuid,
    principal: Uuid,
) -> Result<(Binding, targets::Reached), StatusCode> {
    let (device, opening) =
        previews::authorized(&state.pool, endpoint, principal, state.clock.now())
            .await
            .map_err(|_| StatusCode::FORBIDDEN)?;
    let device = devices::get(&state.pool, device)
        .await
        .map_err(|_| StatusCode::BAD_GATEWAY)?;
    let address = device
        .host
        .parse::<Ipv6Addr>()
        .map_err(|_| StatusCode::BAD_GATEWAY)?;
    let stream = targets::preview(&state.policy, address, &opening)
        .await
        .map_err(|_| StatusCode::BAD_GATEWAY)?;
    Ok((opening.binding, stream))
}

struct Relay {
    state: Browser,
    binding: Binding,
    slot: Arc<OwnedSemaphorePermit>,
}

impl Relay {
    async fn bridge(self, client: hyper::upgrade::OnUpgrade, upstream: hyper::upgrade::OnUpgrade) {
        let _slot = self.slot;
        let carrying = async {
            let (Ok(client), Ok(upstream)) = tokio::join!(client, upstream) else {
                return;
            };
            let _ = tokio::io::copy_bidirectional(
                &mut TokioIo::new(client),
                &mut TokioIo::new(upstream),
            )
            .await;
        };
        tokio::select! { () = carrying => (), () = revoked(&self.state, &self.binding) => () }
    }
}

fn same_origin(request: &Request, expected: &str) -> bool {
    let mut origins = request.headers().get_all(header::ORIGIN).iter();
    match origins.next() {
        Some(value) => origins.next().is_none() && value.as_bytes() == expected.as_bytes(),
        None => {
            (request.method() == Method::GET || request.method() == Method::HEAD)
                && !request.headers().contains_key(header::UPGRADE)
                && request
                    .headers()
                    .get("sec-fetch-site")
                    .is_none_or(|site| site == "same-origin" || site == "none")
        }
    }
}

pub(super) async fn revoked(state: &Browser, expected: &Binding) {
    let remaining = expected
        .expires_at
        .saturating_sub(u64::try_from(state.clock.now().timestamp()).unwrap_or(0))
        .min(600);
    let deadline = tokio::time::Instant::now() + Duration::from_secs(remaining);
    loop {
        let checked = tokio::time::timeout(
            Duration::from_secs(2),
            previews::authorized(
                &state.pool,
                expected.endpoint,
                expected.principal,
                state.clock.now(),
            ),
        )
        .await;
        if !matches!(checked, Ok(Ok((_, ref open))) if open.binding == *expected) {
            return;
        }
        tokio::select! {
            () = tokio::time::sleep_until(deadline) => return,
            () = tokio::time::sleep(Duration::from_secs(1)) => (),
        }
    }
}
