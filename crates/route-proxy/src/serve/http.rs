//! The plain HTTP listener (port 80): the ACME challenge of a name and the redirect to
//! `https://` of a public name (Section 12.2). It never carries the content of an application.

use std::{
    net::IpAddr,
    sync::{Arc, PoisonError},
};

use tokio::{
    io::AsyncWriteExt as _,
    net::{TcpListener, TcpStream},
    time::timeout,
};

use super::Proxy;
use crate::{hub::Hub, limits::Pass, plain, relay::read_head, wire::Reach};

/// Serves plain HTTP on `listener` until the task is stopped.
pub async fn serve<H: Hub>(proxy: Arc<Proxy<H>>, listener: TcpListener) {
    loop {
        let Ok((stream, peer)) = listener.accept().await else {
            continue;
        };
        let Some(pass) = Pass::enter(&proxy.plain_gate) else {
            continue;
        };
        let proxy = Arc::clone(&proxy);
        tokio::spawn(async move { one(&proxy, stream, peer.ip(), pass).await });
    }
}

/// One plain connection: the challenge or the redirect, each step within its time.
async fn one<H: Hub>(proxy: &Proxy<H>, mut stream: TcpStream, peer: IpAddr, mut pass: Pass) {
    let times = proxy.timeouts;
    let Some(client) = proxy.client(&mut stream, peer).await else {
        return;
    };
    if !pass.admit(client) {
        return;
    }
    let Ok(Ok((head, _))) = timeout(times.head, read_head(&mut stream)).await else {
        return;
    };
    let Some((host, target)) = plain::host_and_target(&head) else {
        return;
    };
    let names: Vec<String> = (proxy.names.read().unwrap_or_else(PoisonError::into_inner))
        .iter()
        .filter(|(_, entry)| entry.reach == Reach::Public)
        .map(|(name, _)| name.clone())
        .collect();
    let challenges = proxy
        .challenges
        .read()
        .unwrap_or_else(PoisonError::into_inner)
        .clone();
    let answer = plain::answer(&host, &target, &names, &challenges);
    timeout(times.idle, stream.write_all(&answer)).await.ok();
}
