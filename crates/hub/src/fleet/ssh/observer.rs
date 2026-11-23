//! The hub's side of an SSH key exchange (SEC-007, K4 Section 8.3).
//! The presented key must be the expected one, and every channel the device opens towards the hub is refused (K4 Section 8.4).

use russh::{
    Channel, ChannelOpenFailure,
    client::{self, ChannelOpenHandle, Msg, Session},
    keys::PublicKeyOrCertificate,
};
use tokio::sync::oneshot;

use crate::fleet::trust::PublicKey;

/// A certificate, or a key the hub does not accept, is reported as `None` and never matches (SEC-007).
pub(crate) struct Observer {
    expected: Option<PublicKey>,
    seen: Option<oneshot::Sender<Option<PublicKey>>>,
}

impl Observer {
    /// The receiver fails when no key exchange reached the key.
    pub(crate) fn expecting(
        expected: Option<PublicKey>,
    ) -> (Self, oneshot::Receiver<Option<PublicKey>>) {
        let (seen, receiver) = oneshot::channel();
        let observer = Self {
            expected,
            seen: Some(seen),
        };
        (observer, receiver)
    }

    fn presented(presented: &PublicKeyOrCertificate) -> Option<PublicKey> {
        let PublicKeyOrCertificate::PublicKey { key, .. } = presented else {
            return None;
        };
        PublicKey::from_blob(key.to_bytes().ok()?).ok()
    }
}

async fn refused(channel: Channel<Msg>, reply: ChannelOpenHandle) -> Result<(), russh::Error> {
    drop(channel);
    reply
        .reject(ChannelOpenFailure::AdministrativelyProhibited)
        .await;
    Ok(())
}

impl client::Handler for Observer {
    type Error = russh::Error;

    /// russh verifies the signature first and calls this for the first exchange only; a rekey runs inside the encrypted connection.
    fn check_server_key(
        &mut self,
        server_public_key: &PublicKeyOrCertificate,
    ) -> impl Future<Output = Result<bool, Self::Error>> + Send {
        let presented = Self::presented(server_public_key);
        let go_on = presented.is_some() && presented == self.expected;
        if let Some(seen) = self.seen.take() {
            // The receiver is gone only when the caller was dropped; nobody is left to tell.
            let _gone = seen.send(presented);
        }
        std::future::ready(Ok(go_on))
    }

    fn server_channel_open_session(
        &mut self,
        channel: Channel<Msg>,
        reply: ChannelOpenHandle,
        _session: &mut Session,
    ) -> impl Future<Output = Result<(), Self::Error>> + Send {
        refused(channel, reply)
    }

    fn server_channel_open_x11(
        &mut self,
        channel: Channel<Msg>,
        _originator_address: &str,
        _originator_port: u32,
        reply: ChannelOpenHandle,
        _session: &mut Session,
    ) -> impl Future<Output = Result<(), Self::Error>> + Send {
        refused(channel, reply)
    }

    fn server_channel_open_agent_forward(
        &mut self,
        channel: Channel<Msg>,
        reply: ChannelOpenHandle,
        _session: &mut Session,
    ) -> impl Future<Output = Result<(), Self::Error>> + Send {
        refused(channel, reply)
    }

    fn server_channel_open_forwarded_tcpip(
        &mut self,
        channel: Channel<Msg>,
        _connected_address: &str,
        _connected_port: u32,
        _originator_address: &str,
        _originator_port: u32,
        reply: ChannelOpenHandle,
        _session: &mut Session,
    ) -> impl Future<Output = Result<(), Self::Error>> + Send {
        refused(channel, reply)
    }

    fn server_channel_open_direct_tcpip(
        &mut self,
        channel: Channel<Msg>,
        _host_to_connect: &str,
        _port_to_connect: u32,
        _originator_address: &str,
        _originator_port: u32,
        reply: ChannelOpenHandle,
        _session: &mut Session,
    ) -> impl Future<Output = Result<(), Self::Error>> + Send {
        refused(channel, reply)
    }

    fn server_channel_open_forwarded_streamlocal(
        &mut self,
        channel: Channel<Msg>,
        _socket_path: &str,
        reply: ChannelOpenHandle,
        _session: &mut Session,
    ) -> impl Future<Output = Result<(), Self::Error>> + Send {
        refused(channel, reply)
    }

    fn server_channel_open_direct_streamlocal(
        &mut self,
        channel: Channel<Msg>,
        _socket_path: &str,
        reply: ChannelOpenHandle,
        _session: &mut Session,
    ) -> impl Future<Output = Result<(), Self::Error>> + Send {
        refused(channel, reply)
    }

    /// Unknown channel types stay refused, as russh's default does, without relying on that default.
    fn should_accept_unknown_server_channel(
        &mut self,
        _id: russh::ChannelId,
        _channel_type: &str,
    ) -> impl Future<Output = bool> + Send {
        std::future::ready(false)
    }
}
