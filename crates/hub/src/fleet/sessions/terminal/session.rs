//! The terminal's part of the session task (K4 Section 8); `upgrade::run_session` owns the ticket from redeem to end.
//! A client that sends a byte after the headers and leaves is not seen until the upgrade never comes (SEC-002).

use axum::extract::ws::WebSocket;
use tokio::sync::oneshot;

use super::{
    ending::NOT_TERMINAL,
    opening::{self, Opening},
    route::{Terminal, Timing},
    shell,
    writes::{close_client, leave_unstarted},
};
use crate::fleet::sessions::{
    PendingTicket, Tunnel,
    end::{Cause, End},
    upgrade::{FarSide, upgraded},
};

impl FarSide for Terminal {
    type Opening = Opening;
    const NOT_OFFERED: &'static str = NOT_TERMINAL;

    async fn open(&self, ticket: &PendingTicket) -> Result<Opening, End> {
        let session = ticket.session;
        opening::open(self, ticket).await.map_err(|not_opened| {
            tracing::warn!(%session, error = %not_opened, "the terminal did not open");
            End::explained(Cause::GatewayError, not_opened.message())
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
    match opening {
        Opening::SignedIn(signed_in) => match upgraded(tunnel, socket, timing.upgrade).await {
            Ok(client) => shell::run(client, signed_in, tunnel, timing).await,
            Err(end) => {
                leave_unstarted(signed_in).await;
                end
            }
        },
        Opening::HostKeyChanged => {
            let end = End::from(Cause::HostKeyMismatch);
            refused(end, tunnel, socket, timing).await
        }
        Opening::SignInRefused(message) => {
            let end = End::explained(Cause::AuthFailed, message);
            refused(end, tunnel, socket, timing).await
        }
    }
}

/// The end is the cause even if the upgrade fails: it came first (K4 Section 8.3).
async fn refused(
    end: End,
    tunnel: &mut Tunnel,
    socket: oneshot::Receiver<WebSocket>,
    timing: Timing,
) -> End {
    if let Ok(client) = upgraded(tunnel, socket, timing.upgrade).await {
        close_client(client, end.cause).await;
    }
    end
}
