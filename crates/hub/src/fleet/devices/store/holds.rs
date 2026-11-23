//! Another domain's transaction locks the device row before its own rows, as `delete_device` does.

use sqlx::PgConnection;

use crate::{
    fleet::devices::{Error, MetricsSource, Missing},
    ids::DeviceId,
};

pub(in crate::fleet::devices) async fn lock_device(
    conn: &mut PgConnection,
    id: DeviceId,
) -> Result<(), Error> {
    let row: Option<DeviceId> =
        sqlx::query_scalar("SELECT id FROM devices.devices WHERE id = $1 FOR UPDATE")
            .bind(id)
            .fetch_optional(conn)
            .await?;
    row.map(|_| ()).ok_or(Error::NotFound(Missing::Device(id)))
}

pub(in crate::fleet::devices) async fn lock_metrics_source(
    conn: &mut PgConnection,
    id: DeviceId,
) -> Result<MetricsSource, Error> {
    let row: Option<String> =
        sqlx::query_scalar("SELECT metrics_source FROM devices.devices WHERE id = $1 FOR UPDATE")
            .bind(id)
            .fetch_optional(conn)
            .await?;
    let text = row.ok_or(Error::NotFound(Missing::Device(id)))?;
    text.parse()
        .map_err(|error| Error::Database(sqlx::Error::Decode(Box::new(error))))
}
