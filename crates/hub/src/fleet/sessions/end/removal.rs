use sqlx::{PgPool, Postgres, Transaction};
use tokio::time::timeout;

use super::{Actor, Error, Live, end_by_operator, live, store};
use crate::ids::{DeviceId, SessionId};

pub(crate) struct DeviceSessions {
    device: DeviceId,
    sessions: Vec<SessionId>,
}

impl DeviceSessions {
    // A concurrent start either already has a row or waits and sees the missing device.
    pub(crate) async fn prepare(
        tx: &mut Transaction<'_, Postgres>,
        device: DeviceId,
    ) -> Result<Self, Error> {
        store::lock_starts(tx, device).await?;
        let sessions = store::live_ids(tx, device).await?;
        Ok(Self { device, sessions })
    }

    pub(crate) async fn close(self, db: &PgPool, live: &Live, by: &Actor) {
        let mut tunnels = Vec::new();
        let mut unopened = Vec::new();
        let mut withdrawn = Vec::new();
        // Signal all tunnels before awaiting any database write or tunnel teardown.
        for session in self.sessions {
            match live.end_by(session, by) {
                live::Ending::Signalled(closing) => tunnels.push(closing),
                live::Ending::Withdrawn(ticket) => {
                    unopened.push(session);
                    withdrawn.push(ticket);
                }
                live::Ending::Neither => unopened.push(session),
            }
        }
        if !unopened.is_empty()
            && let Err(error) = end_by_operator(db, live, &unopened, by).await
        {
            // Restoring a spent ticket lets the sweep retry the row, with no usable token.
            live.restore(withdrawn);
            tracing::error!(device = %self.device, %error, "could not record unopened sessions on device removal");
        }
        let _ = timeout(live.tunnel_end_wait(), async {
            for tunnel in tunnels {
                tunnel.finished().await;
            }
        })
        .await;
    }
}
