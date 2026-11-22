//! One listener of a UDP stream, only on a hub that reaches the machine on its own network
//! (OD-HOST-48). Each source address is one flow through the hub, with a limit of flows for each
//! stream and an idle time. A source address of UDP can be false, so the proxy sends to a source
//! at most as many bytes as it received from it (Section 16.4, SF-248), and the new flows of a
//! stream each second and from each source network are limited, so false sources cannot fill its
//! flows (SF-327).

use std::{
    collections::HashMap,
    net::SocketAddr,
    sync::{Arc, Mutex, PoisonError},
};

use tokio::{
    net::UdpSocket,
    sync::mpsc::{self, Receiver, Sender},
    time::Instant,
};

use super::datagrams::{DATAGRAM_MAX, Moved, read_frame, write_frame};
use crate::{
    hub::Hub,
    limits::{Bucket, Pass, source_network},
    serve::Proxy,
    wire::{Dial, StreamRoute, Transport},
};

/// The datagrams of one flow that wait for the hub; more are dropped, as UDP may.
const QUEUE: usize = 64;

type Flows = Arc<Mutex<HashMap<SocketAddr, Sender<Vec<u8>>>>>;

/// Whether `length` more bytes may go to a source that sent `received` bytes and was sent
/// `sent`: never more than it sent, so a false source address cannot be made a target.
#[must_use]
pub const fn may_answer(received: u64, sent: u64, length: usize) -> bool {
    sent.saturating_add(length as u64) <= received
}

/// Receives the datagrams of `stream` on `socket` until the task is stopped.
pub async fn serve<H: Hub>(proxy: Arc<Proxy<H>>, socket: UdpSocket, stream: StreamRoute) {
    let socket = Arc::new(socket);
    let stream = Arc::new(stream);
    let flows: Flows = Arc::default();
    let per_second = proxy.udp_new_per_second();
    let mut arrivals = Bucket::full(per_second, per_second.saturating_mul(2), 0);
    let started = Instant::now();
    let mut buffer = vec![0_u8; DATAGRAM_MAX];
    loop {
        let Ok((length, source)) = socket.recv_from(&mut buffer).await else {
            continue;
        };
        let datagram = buffer[..length].to_vec();
        let known = flows
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(&source)
            .cloned();
        if let Some(sender) = known {
            sender.try_send(datagram).ok();
            continue;
        }
        let now_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
        if !arrivals.take(now_ms) {
            continue;
        }
        let Some(receiver) = open(&proxy, &flows, &stream, source) else {
            continue;
        };
        let Some(pass) = Pass::enter(proxy.stream_gate()) else {
            flows
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .remove(&source);
            continue;
        };
        let flow = Flow {
            proxy: Arc::clone(&proxy),
            socket: Arc::clone(&socket),
            stream: Arc::clone(&stream),
            flows: Arc::clone(&flows),
            source,
        };
        tokio::spawn(flow.run(receiver, datagram, pass));
    }
}

/// A new flow of `source` in `flows`, within the limit of flows of the stream and of the network
/// of the source; `None` when it is refused.
fn open<H: Hub>(
    proxy: &Proxy<H>,
    flows: &Flows,
    stream: &StreamRoute,
    source: SocketAddr,
) -> Option<Receiver<Vec<u8>>> {
    if !proxy.reaches(stream.reach, source.ip()) {
        return None;
    }
    let mut flows = flows.lock().unwrap_or_else(PoisonError::into_inner);
    let network = source_network(source.ip());
    let of_network = flows
        .keys()
        .filter(|known| source_network(known.ip()) == network)
        .count();
    if flows.len() >= proxy.udp_flows() || of_network >= proxy.udp_flows_per_network() {
        return None;
    }
    let (sender, receiver) = mpsc::channel(QUEUE);
    flows.insert(source, sender);
    Some(receiver)
}

/// One flow of one source through the hub.
struct Flow<H: Hub> {
    proxy: Arc<Proxy<H>>,
    socket: Arc<UdpSocket>,
    stream: Arc<StreamRoute>,
    flows: Flows,
    source: SocketAddr,
}

impl<H: Hub> Flow<H> {
    async fn run(self, receiver: Receiver<Vec<u8>>, first: Vec<u8>, mut pass: Pass) {
        let name = &self.stream.name;
        if pass.admit(self.source.ip()) && self.proxy.admit(name) {
            let moved = Moved::default();
            self.carry(receiver, first, &moved).await;
            let (bytes_in, bytes_out) = moved.totals();
            self.proxy.moved(name, bytes_in, bytes_out);
            self.proxy.release(name);
        }
        self.flows
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .remove(&self.source);
    }

    /// Carries the datagrams both ways until the hub ends the flow or it is idle.
    async fn carry(&self, mut receiver: Receiver<Vec<u8>>, first: Vec<u8>, moved: &Moved) {
        let dial = Dial {
            application: self.stream.application,
            name: self.stream.name.clone(),
            transport: Transport::Udp,
        };
        let Ok(app) = self.proxy.hub().connect(dial).await else {
            return;
        };
        let (mut reader, mut writer) = tokio::io::split(app);
        let out = async {
            let mut next = Some(first);
            while let Some(datagram) = next {
                if write_frame(&mut writer, &datagram).await.is_err() {
                    return;
                }
                moved.sent(datagram.len());
                next = receiver.recv().await;
            }
        };
        let back = async {
            let mut frame = Vec::with_capacity(DATAGRAM_MAX);
            while let Ok(Some(length)) = read_frame(&mut reader, &mut frame).await {
                let (from_source, to_source) = moved.totals();
                if !may_answer(from_source, to_source, length) {
                    continue;
                }
                if self
                    .socket
                    .send_to(&frame[..length], self.source)
                    .await
                    .is_err()
                {
                    return;
                }
                moved.received(length);
            }
        };
        tokio::select! {
            () = out => {}
            () = back => {}
            () = moved.quiet(self.proxy.udp_idle()) => {}
        }
    }
}
