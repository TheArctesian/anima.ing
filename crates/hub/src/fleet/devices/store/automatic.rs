//! Initial defaults advertised by an authenticated daemon over its existing private link.

use daemon_core::inventory::RemoteAccess;
use sqlx::PgConnection;

use crate::{fleet::devices::Error, ids::DeviceId};

pub(in crate::fleet::devices) async fn configure(
    conn: &mut PgConnection,
    device: DeviceId,
    access: &RemoteAccess,
) -> Result<bool, Error> {
    let mut changed = false;
    if let Some(user) = &access.ssh_user {
        changed |= sqlx::query(
            "UPDATE devices.ssh_settings SET enabled = true, username = $2, auth = 'password', configured = true \
             WHERE device_id = $1 AND NOT configured AND NOT enabled AND username = '' \
               AND port = 22",
        )
        .bind(device)
        .bind(user)
        .execute(&mut *conn)
        .await?.rows_affected() > 0;
    }
    if let Some(user) = &access.rdp_user {
        changed |= sqlx::query(
            "UPDATE devices.rdp_settings SET enabled = true, username = $2, security = 'tls', \
                 audio_out = false, configured = true \
             WHERE device_id = $1 AND NOT configured AND NOT enabled AND username = '' \
               AND port = 3389 AND NOT ignore_certificate",
        )
        .bind(device)
        .bind(user)
        .execute(&mut *conn)
        .await?
        .rows_affected()
            > 0;
    }
    Ok(changed)
}

pub(in crate::fleet::devices) async fn configure_vnc(
    conn: &mut PgConnection,
    device: DeviceId,
    user: &str,
) -> Result<bool, Error> {
    Ok(sqlx::query(
        "UPDATE devices.vnc_settings SET enabled = true, username = $2, configured = true \
         WHERE device_id = $1 AND NOT configured AND NOT enabled AND username IS NULL AND port = 5900",
    ).bind(device).bind(user).execute(conn).await?.rows_affected() > 0)
}
