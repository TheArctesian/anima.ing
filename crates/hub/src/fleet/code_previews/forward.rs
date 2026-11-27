//! Native TCP forwarding uses the same authorized workspace connector as browser previews.
use super::{Browser, protect, proxy, refusal};
use crate::fleet::agent_threads::managed::browser_sessions::Audience;
use agent_threads::managed::preview::{FORWARD_CHUNK, FORWARD_PROTOCOL, ForwardSession};
use axum::{
    Json,
    body::{Bytes, to_bytes},
    extract::{
        FromRequestParts as _, Request, WebSocketUpgrade,
        ws::{Message, WebSocket},
    },
    http::{HeaderMap, Method, StatusCode, header},
    response::{IntoResponse as _, Response},
};
use futures_util::{SinkExt as _, StreamExt as _};
use std::{io, sync::Arc};
use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};
use uuid::Uuid;

pub(super) async fn serve(state: Browser, endpoint: Uuid, request: Request) -> Response {
    let headers = request.headers();
    if request.uri().query().is_some()
        || headers.contains_key(header::ORIGIN)
        || headers.contains_key(header::COOKIE)
    {
        return refusal(StatusCode::FORBIDDEN);
    }
    match *request.method() {
        Method::POST => redeem(&state, endpoint, request).await,
        Method::GET => open(state, endpoint, request).await,
        Method::HEAD => check(&state, endpoint, headers).await,
        _ => refusal(StatusCode::METHOD_NOT_ALLOWED),
    }
}

async fn redeem(state: &Browser, endpoint: Uuid, request: Request) -> Response {
    if request.headers().contains_key(header::AUTHORIZATION)
        || request
            .headers()
            .get(header::CONTENT_TYPE)
            .is_none_or(|value| value != "application/octet-stream")
    {
        return refusal(StatusCode::BAD_REQUEST);
    }
    let Ok(bytes) = to_bytes(request.into_body(), 43).await else {
        return refusal(StatusCode::BAD_REQUEST);
    };
    let Ok(handoff) = std::str::from_utf8(&bytes) else {
        return refusal(StatusCode::BAD_REQUEST);
    };
    let Ok((token, seconds)) = Audience::Forward
        .redeem(&state.pool, (endpoint, handoff), state.clock.now())
        .await
    else {
        return refusal(StatusCode::UNAUTHORIZED);
    };
    let Ok(seconds) = u64::try_from(seconds) else {
        return refusal(StatusCode::UNAUTHORIZED);
    };
    let mut response = Json(ForwardSession { token, seconds }).into_response();
    protect(&mut response);
    response
}

fn authorization(headers: &HeaderMap) -> Option<&str> {
    let mut values = headers.get_all(header::AUTHORIZATION).iter();
    let token = values.next()?.to_str().ok()?.strip_prefix("Bearer ")?;
    values.next().is_none().then_some(token)
}

async fn check(state: &Browser, endpoint: Uuid, headers: &HeaderMap) -> Response {
    let status = if principal(state, endpoint, headers).await.is_some() {
        StatusCode::NO_CONTENT
    } else {
        StatusCode::UNAUTHORIZED
    };
    refusal(status)
}

async fn principal(state: &Browser, endpoint: Uuid, headers: &HeaderMap) -> Option<Uuid> {
    let token = authorization(headers)?;
    Audience::Forward
        .principal(&state.pool, (endpoint, token), state.clock.now())
        .await
        .ok()
}

async fn open(state: Browser, endpoint: Uuid, request: Request) -> Response {
    let mut protocols = request
        .headers()
        .get_all(header::SEC_WEBSOCKET_PROTOCOL)
        .iter();
    if protocols
        .next()
        .is_none_or(|value| value != FORWARD_PROTOCOL)
        || protocols.next().is_some()
    {
        return refusal(StatusCode::BAD_REQUEST);
    }
    let Ok(slot) = Arc::clone(&state.slots).try_acquire_owned() else {
        return refusal(StatusCode::TOO_MANY_REQUESTS);
    };
    let Some(principal) = principal(&state, endpoint, request.headers()).await else {
        return refusal(StatusCode::UNAUTHORIZED);
    };
    let (mut parts, _) = request.into_parts();
    let Ok(upgrade) = WebSocketUpgrade::from_request_parts(&mut parts, &()).await else {
        return refusal(StatusCode::BAD_REQUEST);
    };
    let (binding, stream) = match proxy::connect_grant(&state, endpoint, principal).await {
        Ok(connected) => connected,
        Err(status) => return refusal(status),
    };
    upgrade
        .protocols([FORWARD_PROTOCOL])
        .max_message_size(FORWARD_CHUNK)
        .max_frame_size(FORWARD_CHUNK)
        .read_buffer_size(FORWARD_CHUNK)
        .write_buffer_size(FORWARD_CHUNK)
        .max_write_buffer_size(3 * FORWARD_CHUNK)
        .on_upgrade(move |socket| async move {
            let _slot = slot;
            tokio::select! {
                biased;
                () = proxy::revoked(&state, &binding) => (),
                _ = relay(socket, stream) => (),
            }
        })
}

async fn relay(
    socket: WebSocket,
    stream: impl tokio::io::AsyncRead + tokio::io::AsyncWrite,
) -> Result<(), io::Error> {
    let broken = |_| io::Error::from(io::ErrorKind::ConnectionAborted);
    let (mut sender, mut receiver) = socket.split();
    let (mut read, mut write) = tokio::io::split(stream);
    let upstream = async {
        while let Some(message) = receiver.next().await {
            match message.map_err(broken)? {
                Message::Binary(bytes) if bytes.is_empty() => return write.shutdown().await,
                Message::Binary(bytes) => write.write_all(&bytes).await?,
                Message::Ping(_) | Message::Pong(_) => (),
                Message::Text(_) | Message::Close(_) => break,
            }
        }
        Err(io::ErrorKind::ConnectionAborted.into())
    };
    let downstream = async {
        let mut buffer = vec![0; FORWARD_CHUNK];
        loop {
            let count = read.read(&mut buffer).await?;
            sender
                .send(Message::Binary(Bytes::copy_from_slice(&buffer[..count])))
                .await
                .map_err(broken)?;
            if count == 0 {
                return Ok(());
            }
        }
    };
    tokio::try_join!(upstream, downstream)?;
    Ok(())
}

#[cfg(test)]
mod tests;
