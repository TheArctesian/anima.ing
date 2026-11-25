use chrono::{DateTime, Utc};
use serde_json::{Value, json};
use sqlx::PgConnection;

use super::{Error, Owner};
use crate::{
    fleet::events::{self, AuditAction, AuditTarget, ClientAddr, NewAudit, Operator},
    ids::DeviceId,
};

pub(in crate::fleet) type Actor = (Operator, ClientAddr);

pub(super) async fn write(
    conn: &mut PgConnection,
    (owner, at): (Owner, DateTime<Utc>),
    (operator, client): Actor,
    (action, change): (AuditAction, Value),
) -> Result<(), Error> {
    events::audit(
        conn,
        at,
        NewAudit {
            operator,
            client,
            action,
            target: AuditTarget::Device(DeviceId::from(owner.device)),
            change: json!({ "owner": owner.account, "remoteThread": change }),
        },
    )
    .await
    .map_err(Error::Audit)
}
