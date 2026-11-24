//! The TLS listeners of the names that serve (Sections 12.3 and 16.7). A private name answers only on
//! the private listener; a connection holds a place of its route until it ends, a WebSocket included.

use std::{
    net::IpAddr,
    sync::{Arc, PoisonError},
};

use tokio::{
    io::{AsyncRead, AsyncWrite, AsyncWriteExt as _},
    net::{TcpListener, TcpStream},
    time::timeout,
};
use tokio_rustls::TlsAcceptor;
use uuid::Uuid;

use super::Proxy;
use crate::{
    hub::Hub,
    limits::Pass,
    plain,
    relay::{Body, Idle, read_head, read_request, relay},
    wire::{Dial, Reach, Transport},
};

const OVER_LIMIT: &[u8] =
    b"HTTP/1.1 503 Service Unavailable\r\ncontent-length: 0\r\nconnection: close\r\n\r\n";
const NO_APPLICATION: &[u8] =
    b"HTTP/1.1 502 Bad Gateway\r\ncontent-length: 0\r\nconnection: close\r\n\r\n";

/// Serves TLS on `listener`, the listener of `reach`, until the task is stopped. A connection
/// over the cap is closed before any byte is read.
pub async fn serve<H: Hub>(proxy: Arc<Proxy<H>>, listener: TcpListener, reach: Reach) {
    loop {
        let Ok((stream, peer)) = listener.accept().await else {
            continue;
        };
        let Some(pass) = Pass::enter(&proxy.tls_gate) else {
            continue;
        };
        let proxy = Arc::clone(&proxy);
        tokio::spawn(async move { one(&proxy, stream, (peer.ip(), reach), pass).await });
    }
}

async fn one<H: Hub>(
    proxy: &Proxy<H>,
    mut stream: TcpStream,
    (peer, listener): (IpAddr, Reach),
    mut pass: Pass,
) {
    let times = proxy.timeouts;
    let started = tokio::time::Instant::now();
    let Some(client) = proxy.client(&mut stream, peer).await else {
        return;
    };
    if !proxy.reaches(listener, client) || !pass.admit(client) {
        return;
    }
    let config = Arc::clone(&proxy.tls.read().unwrap_or_else(PoisonError::into_inner));
    let left = times.handshake.saturating_sub(started.elapsed());
    let Ok(Ok(tls)) = timeout(left, TlsAcceptor::from(config).accept(stream)).await else {
        return;
    };
    let Some(name) = tls.get_ref().1.server_name().map(str::to_owned) else {
        return;
    };
    let tls = Idle::new(tls, times.idle).writes_within(times.write);
    answer(proxy, tls, (&name, client), listener).await;
}

pub(crate) async fn answer<H: Hub, S: AsyncRead + AsyncWrite + Unpin + Send>(
    proxy: &Proxy<H>,
    mut tls: S,
    (name, client): (&str, IpAddr),
    listener: Reach,
) {
    let Some(entry) = proxy.entry(name) else {
        return;
    };
    if listener == Reach::Public && entry.reach == Reach::Private {
        return;
    }
    if !proxy.admit(name) {
        tls.write_all(OVER_LIMIT).await.ok();
        return;
    }
    let status = match &entry.redirect_to {
        Some(to) => redirect(proxy, &mut tls, name, to).await,
        None => request(proxy, &mut tls, (name, entry.application), client).await,
    };
    proxy.release(name);
    tracing::info!(name = %name, status = ?status, "a request of a public name");
}

/// Answers a redirect with its one address; it never reaches a machine.
async fn redirect<H: Hub, S: AsyncRead + AsyncWrite + Unpin + Send>(
    proxy: &Proxy<H>,
    tls: &mut S,
    name: &str,
    to: &str,
) -> Option<u16> {
    let Ok(Ok((head, _))) = timeout(proxy.timeouts.head, read_head(tls)).await else {
        return None;
    };
    let (status, answer) = plain::redirect(&head, to);
    tls.write_all(&answer).await.ok()?;
    proxy.moved(name, head.len() as u64, answer.len() as u64);
    Some(status)
}

async fn request<H: Hub, S: AsyncRead + AsyncWrite + Unpin + Send>(
    proxy: &Proxy<H>,
    tls: &mut S,
    (name, application): (&str, Uuid),
    client: IpAddr,
) -> Option<u16> {
    let by = tokio::time::Instant::now() + proxy.timeouts.request;
    let (rewritten, early) = match read_request(proxy.timeouts.head, tls, name, client).await {
        Ok(read) => read,
        Err(status) => {
            tls.write_all(&status.answer()).await.ok();
            return Some(status.code());
        }
    };
    let dial = Dial {
        application,
        name: name.to_owned(),
        transport: Transport::Tcp,
    };
    let Ok(app) = proxy.hub.connect(dial).await else {
        tls.write_all(NO_APPLICATION).await.ok();
        return Some(502);
    };
    let mut app = Idle::new(app, proxy.timeouts.idle);
    let body = Body { early: &early, by };
    let relayed = relay(tls, body, &mut app, &rewritten).await.ok()?;
    proxy.moved(name, relayed.bytes_in, relayed.bytes_out);
    tracing::debug!(name = %name, bytes_in = relayed.bytes_in, bytes_out = relayed.bytes_out, "relayed");
    relayed.status
}
