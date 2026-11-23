//! Stream listeners follow the hub's table (Section 16.7). In door mode no stream listens
//! (OD-HOST-47); UDP listens only on a hub that a person hosts (OD-HOST-48).

use std::{
    collections::HashMap,
    net::{IpAddr, SocketAddr},
    sync::{Arc, Mutex, PoisonError},
};

use tokio::{
    net::{TcpListener, UdpSocket},
    task::AbortHandle,
};

use super::{tcp, udp};
use crate::{
    hub::Hub,
    serve::Proxy,
    wire::{Reach, StreamRoute, Transport},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Places {
    pub public: IpAddr,
    /// The address on the network of Anima; `None`: no private stream listens.
    pub private: Option<IpAddr>,
    /// The connections come from the door: no stream listens.
    pub door: bool,
}

impl Places {
    /// The address that `stream` listens on; `None` when it does not listen here.
    #[must_use]
    pub fn of(&self, stream: &StreamRoute) -> Option<SocketAddr> {
        if self.door {
            return None;
        }
        let ip = match stream.reach {
            Reach::Public => self.public,
            Reach::Private => self.private?,
        };
        Some(SocketAddr::new(ip, stream.listen_port))
    }
}

struct Running {
    stream: StreamRoute,
    task: AbortHandle,
}

#[derive(Default)]
pub struct Listeners {
    running: Mutex<HashMap<(Transport, u16), Running>>,
}

impl Listeners {
    /// Opens the listeners of `wanted` that do not run and stops those that left or changed.
    /// A port that cannot be bound is left out and logged.
    pub async fn keep<H: Hub>(
        &self,
        proxy: &Arc<Proxy<H>>,
        places: Places,
        wanted: Vec<StreamRoute>,
    ) {
        let wanted: HashMap<(Transport, u16), StreamRoute> = wanted
            .into_iter()
            .filter(|stream| places.of(stream).is_some())
            .map(|stream| ((stream.transport, stream.listen_port), stream))
            .collect();
        let missing: Vec<StreamRoute> = {
            let mut running = self.running.lock().unwrap_or_else(PoisonError::into_inner);
            running.retain(|key, run| {
                let same = wanted.get(key) == Some(&run.stream);
                if !same {
                    run.task.abort();
                }
                same
            });
            wanted
                .iter()
                .filter(|(key, _)| !running.contains_key(key))
                .map(|(_, stream)| stream.clone())
                .collect()
        };
        for stream in missing {
            let Some(address) = places.of(&stream) else {
                continue;
            };
            let Some(task) = listen(proxy, address, stream.clone()).await else {
                tracing::warn!(
                    port = stream.listen_port,
                    "the port of a stream cannot be bound"
                );
                continue;
            };
            let key = (stream.transport, stream.listen_port);
            self.running
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .insert(key, Running { stream, task });
        }
    }
}

async fn listen<H: Hub>(
    proxy: &Arc<Proxy<H>>,
    address: SocketAddr,
    stream: StreamRoute,
) -> Option<AbortHandle> {
    let proxy = Arc::clone(proxy);
    let handle = match stream.transport {
        Transport::Tcp => {
            let listener = TcpListener::bind(address).await.ok()?;
            tokio::spawn(tcp::serve(proxy, listener, stream))
        }
        Transport::Udp => {
            let socket = UdpSocket::bind(address).await.ok()?;
            tokio::spawn(udp::serve(proxy, socket, stream))
        }
    };
    Some(handle.abort_handle())
}
