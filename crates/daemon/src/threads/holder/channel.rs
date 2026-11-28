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
    channel: Option<Channel>,
}

impl Listener {
    pub(super) async fn open(dir: &Path, secure: bool) -> Result<Self, HolderError> {
        let channel = if secure {
            let (accept, root) = holder_channel::acceptor(dir).await.ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "holder identity unavailable",
                )
            })?;
            Some(Channel {
                accept,
                root,
                slots: Arc::new(tokio::sync::Semaphore::new(4)),
            })
        } else {
            None
        };
        Self::bind(dir, channel)
    }

    fn bind(dir: &Path, channel: Option<Channel>) -> Result<Self, HolderError> {
        let (name, stale) = if channel.is_some() {
            (holder_channel::SOCKET, super::SOCKET)
        } else {
            (super::SOCKET, holder_channel::SOCKET)
        };
        match std::fs::remove_file(dir.join(stale)) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
        Ok(Self {
            socket: super::bind(dir, name)?,
            channel,
        })
    }

    pub(super) async fn next(&self) -> io::Result<(UnixStream, Option<Channel>)> {
        Ok((self.socket.accept().await?.0, self.channel.clone()))
    }

    pub(super) fn socket_name(&self) -> &'static str {
        if self.channel.is_some() {
            holder_channel::SOCKET
        } else {
            super::SOCKET
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::AsyncWriteExt as _;
    use tokio::sync::mpsc;

    #[tokio::test]
    async fn a_secure_holder_has_no_plaintext_connection_that_can_replace_the_runner() {
        let dir = tempfile::tempdir().unwrap();
        let legacy = dir.path().join(super::super::SOCKET);
        // A prior holder can leave its socket name behind after a crash.
        drop(UnixListener::bind(&legacy).unwrap());
        let identity = rcgen::generate_simple_self_signed(vec!["holder.invalid".into()]).unwrap();
        let config = rustls::ServerConfig::builder_with_provider(Arc::new(
            rustls::crypto::ring::default_provider(),
        ))
        .with_protocol_versions(&[&rustls::version::TLS13])
        .unwrap()
        .with_no_client_auth()
        .with_single_cert(
            vec![identity.cert.der().clone()],
            rustls::pki_types::PrivatePkcs8KeyDer::from(identity.signing_key.serialize_der())
                .into(),
        )
        .unwrap();
        let channel = Channel {
            accept: TlsAcceptor::from(Arc::new(config)),
            root: identity.cert.der().clone(),
            slots: Arc::new(tokio::sync::Semaphore::new(1)),
        };
        let listener = Listener::bind(dir.path(), Some(channel)).unwrap();
        assert!(UnixStream::connect(&legacy).await.is_err());

        let shared = super::super::holder_state(dir.path(), 0, vec![]);
        let (events, _receiver) = mpsc::unbounded_channel();
        shared.lock().unwrap().connection = Some((1, events));
        let mut client = UnixStream::connect(dir.path().join(holder_channel::SOCKET))
            .await
            .unwrap();
        let (quit, _quitting) = mpsc::channel(1);
        let (stream, channel) = listener.next().await.unwrap();
        let connection = Connection {
            me: 2,
            turn: Arc::default(),
            quit,
        };
        client
            .write_all(b"{\"id\":42,\"action\":\"quit\"}\n")
            .await
            .unwrap();
        client.shutdown().await.unwrap();
        serve(Arc::clone(&shared), (stream, channel), connection).await;
        assert!(shared.lock().unwrap().serves(1));
    }
}
