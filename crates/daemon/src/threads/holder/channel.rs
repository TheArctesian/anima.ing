use std::{
    io,
    path::Path,
    sync::{Arc, Mutex},
    time::Duration,
};

use rustls::pki_types::CertificateDer;
use tokio::net::{UnixListener, UnixStream};
use tokio_rustls::TlsAcceptor;

use super::{Connection, HolderError, Shared};
use crate::threads::holder_channel;

#[derive(Clone)]
pub(super) struct Channel {
    accept: TlsAcceptor,
    root: CertificateDer<'static>,
    slots: Arc<tokio::sync::Semaphore>,
}

pub(super) struct Listener {
    socket: UnixListener,
    channel: Channel,
}

impl Listener {
    pub(super) async fn open(dir: &Path, channel: bool) -> Result<Option<Self>, HolderError> {
        if !channel {
            return Ok(None);
        }
        let Some((accept, root)) = holder_channel::acceptor(dir).await else {
            return Ok(None);
        };
        let socket = super::bind(dir, holder_channel::SOCKET)?;
        let channel = Channel {
            accept,
            root,
            slots: Arc::new(tokio::sync::Semaphore::new(4)),
        };
        Ok(Some(Self { socket, channel }))
    }
}

pub(super) async fn next(
    local: &UnixListener,
    secure: Option<&Listener>,
) -> io::Result<(UnixStream, Option<Channel>)> {
    let local = async { Ok((local.accept().await?.0, None)) };
    let Some(secure) = secure else {
        return local.await;
    };
    tokio::select! {
        result = local => result,
        result = secure.socket.accept() => Ok((result?.0, Some(secure.channel.clone()))),
    }
}

pub(super) async fn serve(
    shared: Arc<Mutex<Shared>>,
    (stream, channel): (UnixStream, Option<Channel>),
    connection: Connection,
) {
    if !stream
        .peer_cred()
        .is_ok_and(|peer| peer.uid() == rustix::process::getuid().as_raw())
    {
        return;
    }
    if let Some(channel) = channel {
        let Ok(slot) = channel.slots.try_acquire() else {
            return;
        };
        let Ok(Ok(stream)) =
            tokio::time::timeout(Duration::from_secs(3), channel.accept.accept(stream)).await
        else {
            return;
        };
        drop(slot);
        super::serve(shared, stream, connection, Some(channel.root)).await;
    } else {
        super::serve(shared, stream, connection, None).await;
    }
}
