//! The Guacamole tunnel's part of the session task (K4 Sections 2 to 5); the shared task
//! (`upgrade::run_session`) owns the ticket from redeem to end.
//!
//! Accepted gap (SEC-002): hyper does not see a client that sends a byte after the request
//! headers and leaves, so the opening carries on until the upgrade never comes.

mod link;
mod writes;

use axum::extract::ws::WebSocket;
use tokio::sync::oneshot;

use super::{
    ending::{Cause, End},
    gateway::certificate_changed,
    opening::{self, Opening},
    route::{Route, Timing},
};
use crate::fleet::sessions::{
    PendingTicket, Tunnel,
    upgrade::{FarSide, close, upgraded},
};
use writes::{leave_gateway, send_text};

const NOT_GUACAMOLE: &str = "The client did not open the tunnel as a Guacamole WebSocket, so \
                             the hub closed the session.";

impl FarSide for Route {
    type Opening = Opening;
    const NOT_OFFERED: &'static str = NOT_GUACAMOLE;

    async fn open(&self, ticket: &PendingTicket) -> Result<Opening, End> {
        let (session, protocol) = (ticket.session, ticket.protocol);
        opening::open(self, ticket).await.map_err(|not_opened| {
            tracing::warn!(%session, error = %not_opened, "the tunnel did not open");
            End::explained(Cause::GatewayError, not_opened.message(protocol))
        })
    }

    async fn connected(
        &self,
        tunnel: &mut Tunnel,
        opening: Opening,
        socket: oneshot::Receiver<WebSocket>,
    ) -> End {
        connected(self.timing, tunnel, opening, socket).await
    }
}

pub(super) async fn connected(
    timing: Timing,
    tunnel: &mut Tunnel,
    opening: Opening,
    socket: oneshot::Receiver<WebSocket>,
) -> End {
    let client = upgraded(tunnel, socket, timing.handshake).await;
    match (opening, client) {
        (Opening::Gateway(opened), Ok(client)) => {
            link::relay(client, *opened, tunnel, timing).await
        }
        (Opening::Gateway(opened), Err(end)) => {
            leave_gateway(opened.connected.stream).await;
            end
        }
        (Opening::CertificateChanged, Ok(client)) => {
            certificate_mismatch(client, tunnel, timing).await
        }
        (Opening::CertificateChanged, Err(_)) => Cause::ServerCertificateMismatch.into(),
    }
}

/// Guacd was never reached (K4 Section 5).
async fn certificate_mismatch(mut client: WebSocket, tunnel: &mut Tunnel, timing: Timing) -> End {
    let cause = Cause::ServerCertificateMismatch;
    let _ = send_text(
        &mut client,
        &certificate_changed(),
        tunnel,
        timing.client_idle,
    )
    .await;
    close(client, None, cause).await;
    cause.into()
}
