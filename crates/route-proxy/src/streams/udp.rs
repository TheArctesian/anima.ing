//! One listener of a UDP stream, only on a hub that reaches the machine on its own network
//! (OD-HOST-48). A UDP source address can be false, so a source gets at most the bytes it sent
//! (Section 16.4, SF-248), and a new flow replaces only a waiting one (SF-327).

use std::{
    net::SocketAddr,
    sync::{Arc, Mutex, PoisonError},
};

use tokio::{
    net::UdpSocket,
    sync::{
        mpsc::{self, Receiver, Sender},
        oneshot,
    },
    time::Instant,
};

use super::{
    datagrams::{DATAGRAM_MAX, Moved, read_frame, write_frame},
    flows::{FlowCaps, FlowTable},
};
use crate::{
    hub::Hub,
    limits::{Bucket, Pass},
    serve::Proxy,
    wire::{Dial, StreamRoute, Transport},
};

/// The datagrams of one flow that wait for the hub; more are dropped, as UDP may.
const QUEUE: usize = 64;

/// When the table drops a handle, its flow ends at once, also while it dials.
struct Handle {
    datagrams: Sender<Vec<u8>>,
    _kept: oneshot::Sender<()>,
}

type Flows = Arc<Mutex<FlowTable<Handle>>>;

#[must_use]
pub const fn may_answer(received: u64, sent: u64, length: usize) -> bool {
    sent.saturating_add(length as u64) <= received
}

pub async fn serve<H: Hub>(proxy: Arc<Proxy<H>>, socket: UdpSocket, stream: StreamRoute) {
    let socket = Arc::new(socket);
    let stream = Arc::new(stream);
    let per_second = proxy.udp_new_per_second();
    let burst = per_second.saturating_mul(2);
    let flows: Flows = Arc::new(Mutex::new(FlowTable::new(flow_caps(
        proxy.udp_flows(),
        proxy.udp_flows_per_network(),
        burst,
        proxy.route_connections(),
    ))));
    let _ends = EndsFlows(Arc::clone(&flows));
    let mut arrivals = Bucket::full(per_second, burst, 0);
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
            .get(source)
            .map(|handle| handle.datagrams.clone());
        if let Some(sender) = known {
            sender.try_send(datagram).ok();
            continue;
        }
        let now_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
        if !arrivals.take(now_ms) {
            continue;
        }
        let Some((id, receiver, dropped)) = open(&proxy, &flows, &stream, source) else {
            continue;
        };
        let Some(pass) = Pass::enter(proxy.stream_gate()) else {
            flows
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .close(source, id);
            continue;
        };
        let flow = Flow {
            proxy: Arc::clone(&proxy),
            socket: Arc::clone(&socket),
            stream: Arc::clone(&stream),
            flows: Arc::clone(&flows),
            source,
            id,
        };
        tokio::spawn(flow.run((receiver, dropped), datagram, pass));
    }
}

/// Waiting and set-up flows hold each connection of the route but one, so a client that replaces
/// a waiting flow gets a connection before the replaced flow gives its own back (SF-327).
#[must_use]
pub fn flow_caps(total: usize, per_network: usize, burst: u32, route_connections: u32) -> FlowCaps {
    let connections = usize::try_from(route_connections).unwrap_or(usize::MAX);
    let burst = usize::try_from(burst).unwrap_or(usize::MAX);
    let waiting = burst
        .min(connections / 2)
        .min(total.saturating_sub(1))
        .max(1);
    let set_up = total
        .saturating_sub(waiting)
        .min(connections.saturating_sub(waiting).saturating_sub(1));
    FlowCaps {
        total,
        per_network,
        waiting,
        set_up,
    }
}

fn open<H: Hub>(
    proxy: &Proxy<H>,
    flows: &Flows,
    stream: &StreamRoute,
    source: SocketAddr,
) -> Option<(u64, Receiver<Vec<u8>>, oneshot::Receiver<()>)> {
    if !proxy.reaches(stream.reach, source.ip()) {
        return None;
    }
    let (datagrams, receiver) = mpsc::channel(QUEUE);
    let (kept, dropped) = oneshot::channel();
    let handle = Handle {
        datagrams,
        _kept: kept,
    };
    let opened = flows
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .open(source, handle)?;
    drop(opened.replaced);
    Some((opened.id, receiver, dropped))
}

/// Its table entry goes when it is dropped, also when its task panics.
struct Flow<H: Hub> {
    proxy: Arc<Proxy<H>>,
    socket: Arc<UdpSocket>,
    stream: Arc<StreamRoute>,
    flows: Flows,
    source: SocketAddr,
    id: u64,
}

impl<H: Hub> Flow<H> {
    async fn run(
        self,
        (receiver, dropped): (Receiver<Vec<u8>>, oneshot::Receiver<()>),
        first: Vec<u8>,
        mut pass: Pass,
    ) {
        let name = &self.stream.name;
        if pass.admit(self.source.ip()) && self.proxy.admit(name) {
            let _admitted = Admitted {
                proxy: &self.proxy,
                name,
            };
            let moved = Moved::default();
            tokio::select! {
                () = self.carry(receiver, first, &moved) => {}
                _ = dropped => {}
            }
            let (bytes_in, bytes_out) = moved.totals();
            self.proxy.moved(name, bytes_in, bytes_out);
        }
    }

    /// Marks the flow set up: its source sent again after the application answered it.
    fn set_up(&self) -> bool {
        self.flows
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .set_up(self.source, self.id)
    }

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
            let mut set_up = false;
            while let Some(datagram) = next {
                if write_frame(&mut writer, &datagram).await.is_err() {
                    return;
                }
                moved.sent(datagram.len());
                next = receiver.recv().await;
                if !set_up && next.is_some() && moved.totals().1 > 0 {
                    set_up = self.set_up();
                }
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

impl<H: Hub> Drop for Flow<H> {
    fn drop(&mut self) {
        self.flows
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .close(self.source, self.id);
    }
}

/// When `serve` is aborted, each flow ends and gives back its connection (SF-327).
struct EndsFlows(Flows);

impl Drop for EndsFlows {
    fn drop(&mut self) {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clear();
    }
}

/// An admitted connection of a name, given back on drop, also when the flow panics.
struct Admitted<'a, H: Hub> {
    proxy: &'a Proxy<H>,
    name: &'a str,
}

impl<H: Hub> Drop for Admitted<'_, H> {
    fn drop(&mut self) {
        self.proxy.release(self.name);
    }
}
