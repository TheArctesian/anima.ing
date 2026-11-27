//! The SQL on `trust.server_certificates`: each device's accepted RDP certificate (SEC-007).

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use sqlx::{PgConnection, PgExecutor};

use super::is_violation_of;
use crate::{
    fleet::trust::{CertificateProtocol, Error, model::ServerCertificate},
    ids::{DeviceId, OperatorId},
};

#[derive(sqlx::FromRow)]
struct ServerCertificateRow {
    device_id: DeviceId,
    fingerprint_sha256: String,
    subject: Option<String>,
    not_after: Option<DateTime<Utc>>,
    first_seen_at: DateTime<Utc>,
}

impl From<ServerCertificateRow> for (DeviceId, ServerCertificate) {
    fn from(row: ServerCertificateRow) -> Self {
        let certificate = ServerCertificate {
            fingerprint_sha256: row.fingerprint_sha256,
            subject: row.subject,
            not_after: row.not_after,
            first_seen_at: row.first_seen_at,
        };
        (row.device_id, certificate)
    }
}

/// The accepted certificate of each listed device that has one, in one query (API-005).
pub(in crate::fleet::trust) async fn server_certificates<'e>(
    db: impl PgExecutor<'e>,
    devices: &[DeviceId],
    protocol: CertificateProtocol,
) -> Result<HashMap<DeviceId, ServerCertificate>, Error> {
    if devices.is_empty() {
        return Ok(HashMap::new());
    }
    let query = format!(
        "select device_id, fingerprint_sha256, subject, not_after, first_seen_at \
         from {} where device_id = any($1)",
        table(protocol)
    );
    let rows: Vec<ServerCertificateRow> =
        sqlx::query_as(&query).bind(devices).fetch_all(db).await?;
    Ok(rows
        .into_iter()
        .map(<(DeviceId, ServerCertificate)>::from)
        .collect())
}

/// Serialises accepts for one device, so each audit entry names the certificate it replaced.
pub(in crate::fleet::trust) async fn lock_server_certificate(
    conn: &mut PgConnection,
    device: DeviceId,
) -> Result<(), Error> {
    sqlx::query(
        "select pg_advisory_xact_lock(\
         hashtextextended('trust.server_certificates:' || $1::text, 0))",
    )
    .bind(device)
    .execute(conn)
    .await?;
    Ok(())
}

pub(in crate::fleet::trust) struct AcceptedCertificate<'a> {
    pub(in crate::fleet::trust) device: DeviceId,
    pub(in crate::fleet::trust) protocol: CertificateProtocol,
    pub(in crate::fleet::trust) certificate: &'a ServerCertificate,
    pub(in crate::fleet::trust) by: OperatorId,
}

pub(in crate::fleet::trust) async fn put_server_certificate(
    conn: &mut PgConnection,
    accepted: &AcceptedCertificate<'_>,
) -> Result<(), Error> {
    let certificate = accepted.certificate;
    let query = format!(
        "insert into {} (device_id, fingerprint_sha256, subject, \
                not_after, first_seen_at, accepted_by) \
         values ($1, $2, $3, $4, $5, $6) \
         on conflict (device_id) do update set fingerprint_sha256 = excluded.fingerprint_sha256, \
             subject = excluded.subject, not_after = excluded.not_after, \
             first_seen_at = excluded.first_seen_at, accepted_by = excluded.accepted_by",
        table(accepted.protocol)
    );
    let stored = sqlx::query(&query)
        .bind(accepted.device)
        .bind(&certificate.fingerprint_sha256)
        .bind(certificate.subject.as_deref())
        .bind(certificate.not_after)
        .bind(certificate.first_seen_at)
        .bind(accepted.by)
        .execute(conn)
        .await;
    match stored {
        Ok(_) => Ok(()),
        Err(error)
            if (is_violation_of(&error, "server_certificates_device_id_fkey")
                || is_violation_of(&error, "vnc_server_certificates_device_id_fkey")) =>
        {
            Err(Error::NotFound(accepted.device))
        }
        Err(error) => Err(error.into()),
    }
}

fn table(protocol: CertificateProtocol) -> &'static str {
    match protocol {
        CertificateProtocol::Rdp => "trust.server_certificates",
        CertificateProtocol::Vnc => "trust.vnc_server_certificates",
    }
}
