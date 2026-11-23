//! The detected operating system never overwrites the operator's choice (DEV-025).

use sqlx::PgPool;

use crate::{
    fleet::devices::{Error, OperatingSystem},
    ids::DeviceId,
};

pub(in crate::fleet::devices) async fn set_detected_os(
    db: &PgPool,
    id: DeviceId,
    os: OperatingSystem,
) -> Result<bool, Error> {
    let done = sqlx::query("UPDATE devices.devices SET os = $2 WHERE id = $1 AND os = 'other'")
        .bind(id)
        .bind(os.as_str())
        .execute(db)
        .await?;
    Ok(done.rows_affected() == 1)
}
