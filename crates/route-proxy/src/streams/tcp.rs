//! A TCP stream listener: each connection reaches its route through the hub and holds a place
//! of the route until it ends.

use std::{net::SocketAddr, sync::Arc};

use tokio::{
    io::copy_bidirectional,
    net::{TcpListener, TcpStream},
};

use crate::{
    hub::Hub,
    limits::Pass,
    relay::Idle,
    serve::Proxy,
    wire::{Dial, StreamRoute, Transport},
};

pub async fn serve<H: Hub>(proxy: Arc<Proxy<H>>, listener: TcpListener, stream: StreamRoute) {
    let stream = Arc::new(stream);
    loop {
        let Ok((client, peer)) = listener.accept().await else {
            continue;
        };
        let Some(pass) = Pass::enter(proxy.stream_gate()) else {
            continue;
        };
        let proxy = Arc::clone(&proxy);
        let stream = Arc::clone(&stream);
        tokio::spawn(async move { one(&proxy, client, (peer, pass), &stream).await });
    }
}

async fn one<H: Hub>(
    proxy: &Proxy<H>,
    mut client: TcpStream,
    (peer, mut pass): (SocketAddr, Pass),
    stream: &StreamRoute,
) {
    if !proxy.reaches(stream.reach, peer.ip()) || !pass.admit(peer.ip()) {
        return;
    }
    if !proxy.admit(&stream.name) {
        return;
    }
    let dial = Dial {
        application: stream.application,
        name: stream.name.clone(),
        transport: Transport::Tcp,
    };
    if let Ok(app) = proxy.hub().connect(dial).await {
        let times = proxy.timeouts();
        let mut app = Idle::new(app, times.idle);
        let mut client = Idle::new(&mut client, times.idle).writes_within(times.write);
        if let Ok((bytes_in, bytes_out)) = copy_bidirectional(&mut client, &mut app).await {
            proxy.moved(&stream.name, bytes_in, bytes_out);
        }
    }
    proxy.release(&stream.name);
    tracing::info!(name = %stream.name, "a connection of a stream ended");
}
