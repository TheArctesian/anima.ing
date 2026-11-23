//! A hub loopback port that carries to one port of a computer on the link, for guacd, which
//! cannot reach a link address (HMD-048); the computer still decides which ports are open (HMD-049).
//!
//! Any program on the hub's server can connect while the port lives: a hub with a link is its
//! server's only user (`anima-local-hub`), and the far end has its own sign-in.

use std::{
    io,
    net::{Ipv4Addr, SocketAddr},
};

use tokio::{
    io::copy_bidirectional,
    net::{TcpListener, TcpStream},
    task::{JoinHandle, JoinSet},
};

use super::{AddressPolicy, connect, on_link};

#[derive(Debug)]
pub struct LinkPort {
    local: SocketAddr,
    carrying: JoinHandle<()>,
}

impl LinkPort {
    /// `None` when `target` is not a link address or the hub has no link.
    ///
    /// # Errors
    /// When no loopback port can be taken.
    pub async fn open(policy: &AddressPolicy, target: SocketAddr) -> io::Result<Option<Self>> {
        if !(on_link(target.ip()) && policy.link_dial().is_some()) {
            return Ok(None);
        }
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await?;
        let local = listener.local_addr()?;
        let carrying = tokio::spawn(carry_all(listener, policy.clone(), target));
        Ok(Some(Self { local, carrying }))
    }

    #[must_use]
    pub const fn local(&self) -> SocketAddr {
        self.local
    }
}

impl Drop for LinkPort {
    /// Closes the port and each connection that it carries.
    fn drop(&mut self) {
        self.carrying.abort();
    }
}

async fn carry_all(listener: TcpListener, policy: AddressPolicy, target: SocketAddr) {
    let mut carried = JoinSet::new();
    loop {
        tokio::select! {
            accepted = listener.accept() => {
                let Ok((local, peer)) = accepted else { return };
                if peer.ip().is_loopback() {
                    carried.spawn(carry(local, policy.clone(), target));
                }
            }
            Some(_) = carried.join_next(), if !carried.is_empty() => {}
        }
    }
}

async fn carry(mut local: TcpStream, policy: AddressPolicy, target: SocketAddr) {
    if let Ok(mut far) = connect(&policy, target).await {
        let _ended = copy_bidirectional(&mut local, &mut far).await;
    }
}
