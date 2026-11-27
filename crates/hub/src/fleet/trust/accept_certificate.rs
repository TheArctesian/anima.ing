//! `acceptServerCertificate` (SEC-007, LOG-005): the certificate and its audit entry are stored
//! in one transaction.

use serde_json::json;
use sqlx::PgPool;

use super::{
    CertificateProtocol, Conflict, Error, find_device,
    model::{Actor, ServerCertificate},
    observations::Endpoint,
    sightings::Sightings,
    store::{self, AcceptedCertificate},
};
use crate::{
    fingerprint::CertificateFingerprint,
    fleet::events::{self, AuditAction, AuditTarget, NewAudit},
    ids::DeviceId,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::fleet::trust) struct CertificateAcceptance {
    pub(in crate::fleet::trust) device: DeviceId,
    pub(in crate::fleet::trust) protocol: CertificateProtocol,
    pub(in crate::fleet::trust) fingerprint: CertificateFingerprint,
}

/// What is stored comes from the connection test's observation, never from the request.
pub(in crate::fleet::trust) async fn accept_server_certificate(
    db: &PgPool,
    sightings: &Sightings,
    by: &Actor,
    acceptance: CertificateAcceptance,
) -> Result<ServerCertificate, Error> {
    let device = find_device(db, acceptance.device).await?;
    let port = match acceptance.protocol {
        CertificateProtocol::Rdp => device.rdp.port,
        CertificateProtocol::Vnc => device.vnc.port,
    };
    let seen = sightings
        .observed_certificate(
            (&Endpoint::new(&device.host, port), acceptance.protocol),
            by.at,
        )
        .ok_or(Error::Conflict(Conflict::CertificateNotSeen))?;
    if seen.presented.fingerprint != acceptance.fingerprint {
        return Err(Error::Conflict(Conflict::CertificateDiffers));
    }
    let mut tx = db.begin().await?;
    store::lock_server_certificate(&mut tx, device.id).await?;
    let old = store::server_certificates(&mut *tx, &[device.id], acceptance.protocol)
        .await?
        .remove(&device.id);
    let certificate = ServerCertificate::from(seen);
    if let Some(old) = old
        .as_ref()
        .filter(|old| old.fingerprint_sha256 == certificate.fingerprint_sha256)
    {
        tx.commit().await?;
        return Ok(old.clone());
    }
    let accepted = AcceptedCertificate {
        device: device.id,
        protocol: acceptance.protocol,
        certificate: &certificate,
        by: by.operator.id,
    };
    store::put_server_certificate(&mut tx, &accepted).await?;
    let certificate_key = match acceptance.protocol {
        CertificateProtocol::Rdp => "serverCertificate",
        CertificateProtocol::Vnc => "vncServerCertificate",
    };
    let change = json!({
        "deviceName": device.name,
        (certificate_key): {
            "oldFingerprint": old.map(|old| old.fingerprint_sha256),
            "newFingerprint": certificate.fingerprint_sha256,
        },
    });
    let audit = NewAudit {
        operator: by.operator.clone(),
        action: AuditAction::Accept,
        target: AuditTarget::Device(device.id),
        client: by.client,
        change,
    };
    events::audit(&mut tx, by.at, audit).await?;
    tx.commit().await?;
    Ok(certificate)
}
