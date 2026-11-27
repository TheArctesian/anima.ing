//! Trusts only the certificate with this fingerprint (SEC-007).

use super::documents::{
    CertificateAccept, VncCertificateAccept, certificate_accept, vnc_certificate_accept,
};
use super::fingerprint;
use crate::devices::get::resolve;
use crate::failure::Failure;
use crate::invocation::Invocation;
use crate::output::Table;

#[derive(Debug, clap::Args)]
pub struct CertificateArgs {
    /// The device's ID or its exact name.
    device: String,
    /// The SHA-256 fingerprint of the certificate, as `anima device test --protocol rdp|vnc` shows it.
    #[arg(value_parser = fingerprint)]
    fingerprint: String,
    /// The protocol whose certificate is being accepted.
    #[arg(long, value_enum, default_value_t = CertificateProtocol::Rdp)]
    protocol: CertificateProtocol,
}

#[derive(Clone, Copy, Debug, clap::ValueEnum)]
enum CertificateProtocol {
    Rdp,
    Vnc,
}

impl CertificateArgs {
    /// Trusts the certificate and prints what was accepted.
    ///
    /// # Errors
    /// [`Failure::NotFound`] for an unknown device; the hub's `CONFLICT` (exit 6) when the
    /// fingerprint is not the certificate the hub saw.
    pub async fn run(self, invocation: &Invocation) -> Result<(), Failure> {
        let hub = invocation.hub()?;
        let device_id = resolve(&hub, &self.device).await?;
        match self.protocol {
            CertificateProtocol::Rdp => {
                let certificate = hub
                    .run::<CertificateAccept>(certificate_accept::Variables {
                        device_id,
                        fingerprint_sha256: self.fingerprint,
                    })
                    .await?
                    .accept_server_certificate;
                invocation.output.note(&format!(
                    "Accepted the server certificate of {}.",
                    self.device
                ));
                invocation.output.data(&certificate, |certificate| {
                    table(
                        &certificate.fingerprint_sha256,
                        certificate.subject.as_deref(),
                        certificate.not_after.as_deref(),
                        &certificate.first_seen_at,
                    )
                })
            }
            CertificateProtocol::Vnc => {
                let certificate = hub
                    .run::<VncCertificateAccept>(vnc_certificate_accept::Variables {
                        device_id,
                        fingerprint_sha256: self.fingerprint,
                    })
                    .await?
                    .accept_vnc_server_certificate;
                invocation.output.note(&format!(
                    "Accepted the server certificate of {}.",
                    self.device
                ));
                invocation.output.data(&certificate, |certificate| {
                    table(
                        &certificate.fingerprint_sha256,
                        certificate.subject.as_deref(),
                        certificate.not_after.as_deref(),
                        &certificate.first_seen_at,
                    )
                })
            }
        }
    }
}

fn table(
    fingerprint: &str,
    subject: Option<&str>,
    not_after: Option<&str>,
    first_seen: &str,
) -> Table {
    let mut table = Table::record();
    table.pair("Fingerprint", fingerprint);
    table.pair("Subject", subject.unwrap_or("-"));
    table.pair("Not after", not_after.unwrap_or("-"));
    table.pair("First seen", first_seen);
    table
}
