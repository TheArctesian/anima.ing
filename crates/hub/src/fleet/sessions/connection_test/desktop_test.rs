//! The connection test for RDP and VNC (DEV-013, SEC-007). No credential leaves the hub:
//! RDP stops before `CredSSP`; VNC stops before Plain authentication.

use chrono::{DateTime, Utc};
use sqlx::PgPool;

use super::{
    rdp::{Reply, read_server_certificate},
    reach::ReadError,
    ssh_test::{ConnectionTest, refused_host},
    vnc::read_certificate,
    x224::{PROTOCOL_RDP, TLS_PROTOCOLS},
};
use crate::{
    fleet::{
        devices::RdpSecurity,
        sessions::{Desktop, Error, find_device, live::Live},
        ssh::{Target, probe::PROBE_TIMEOUT},
        trust::{
            self, CertificateProtocol, Endpoint, Observation, PresentedCertificate,
            ServerCertificate,
        },
    },
    ids::DeviceId,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::fleet::sessions) enum DesktopSettings {
    Rdp { port: u16, security: RdpSecurity },
    Vnc { port: u16 },
}

/// No certificate can be accepted for unsaved values, so any certificate read is new.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::fleet::sessions) enum DesktopSubject {
    Device {
        id: DeviceId,
        protocol: Desktop,
    },
    Unsaved {
        host: String,
        settings: DesktopSettings,
    },
}

pub(in crate::fleet::sessions) const NEW_CERTIFICATE: &str = "The hub has not accepted this server \
     certificate yet. Compare its fingerprint with the device's own, then accept it. An unsaved \
     device must be saved first. The hub does not sign in during a test.";
pub(in crate::fleet::sessions) const CHANGED_CERTIFICATE: &str = "The device presented a different \
     server certificate from the one accepted. Accept the new certificate only if you know why \
     it changed. The hub does not sign in during a test.";
pub(in crate::fleet::sessions) const ACCEPTED_CERTIFICATE: &str = "The server certificate is the \
     accepted one. The hub does not sign in during a test, so the credential was not checked.";
pub(in crate::fleet::sessions) const STANDARD_ONLY: &str = "The device offers only standard RDP \
     security, which has no certificate to check. The hub does not sign in during a test.";
pub(in crate::fleet::sessions) const NEGOTIATION_REFUSED: &str = "The device refused the RDP security \
     the hub asked for. Check the security setting. The hub does not sign in during a test.";
pub(in crate::fleet::sessions) const NO_TLS: &str =
    "The device answered on RDP but did not complete a TLS handshake.";
pub(in crate::fleet::sessions) const VNC_ANSWERED: &str = "The device answered as a VNC server. The \
     hub does not sign in during a test; no credential was sent.";
pub(in crate::fleet::sessions) const UNRESOLVED: &str = "The host name does not resolve.";
pub(in crate::fleet::sessions) const UNREACHABLE: &str = "The device does not answer on that port.";
pub(in crate::fleet::sessions) const TIMED_OUT: &str =
    "The device did not finish within 8 seconds.";
pub(in crate::fleet::sessions) const NOT_RDP: &str = "The device did not answer as an RDP server.";
pub(in crate::fleet::sessions) const NOT_VNC: &str = "The device did not answer as a VNC server.";

enum Seen {
    Certificate(Observation<PresentedCertificate>),
    Standard,
    Refused,
}

/// A presented certificate is kept 15 minutes for `acceptServerCertificate`, whether or not
/// the device ignores certificates.
///
/// # Errors
/// `NotFound` for an unknown device; `Validation` on `host` for a refused address (R-032);
/// `Database` or `Devices` when a dependency fails. An unreachable device is a result.
pub(in crate::fleet::sessions) async fn test_desktop(
    db: &PgPool,
    live: &Live,
    subject: DesktopSubject,
    at: DateTime<Utc>,
) -> Result<ConnectionTest, Error> {
    let (host, settings, accepted) = plan(db, subject).await?;
    match settings {
        DesktopSettings::Rdp { port, security } => {
            let requested = if security == RdpSecurity::Rdp {
                PROTOCOL_RDP
            } else {
                TLS_PROTOCOLS
            };
            let target = target(live, &host, port);
            let read = read_server_certificate(target, requested, PROBE_TIMEOUT).await;
            let seen = read.map(|reply| match reply {
                Reply::Certificate(certificate, _) => {
                    let endpoint = Endpoint::new(&host, port);
                    Seen::Certificate(live.sightings().observe_certificate(
                        (endpoint, CertificateProtocol::Rdp),
                        certificate,
                        at,
                    ))
                }
                Reply::Standard => Seen::Standard,
                Reply::Refused => Seen::Refused,
            });
            rdp_outcome(accepted.as_ref(), seen)
        }
        DesktopSettings::Vnc { port } => {
            let read = read_certificate(target(live, &host, port), PROBE_TIMEOUT).await;
            let seen = read.map(|certificate| {
                certificate.map(|certificate| {
                    live.sightings().observe_certificate(
                        (Endpoint::new(&host, port), CertificateProtocol::Vnc),
                        certificate,
                        at,
                    )
                })
            });
            vnc_outcome(accepted.as_ref(), seen)
        }
    }
}

fn target<'a>(live: &'a Live, host: &'a str, port: u16) -> Target<'a> {
    Target {
        host,
        port,
        policy: live.policy(),
    }
}

async fn plan(
    db: &PgPool,
    subject: DesktopSubject,
) -> Result<(String, DesktopSettings, Option<ServerCertificate>), Error> {
    let (id, protocol) = match subject {
        DesktopSubject::Unsaved { host, settings } => return Ok((host, settings, None)),
        DesktopSubject::Device { id, protocol } => (id, protocol),
    };
    let device = find_device(db, id).await?;
    Ok(match protocol {
        Desktop::Rdp => {
            let accepted = trust::server_certificates(db, &[id]).await?.remove(&id);
            let settings = DesktopSettings::Rdp {
                port: device.rdp.port,
                security: device.rdp.security,
            };
            (device.host, settings, accepted)
        }
        Desktop::Vnc => (
            device.host,
            DesktopSettings::Vnc {
                port: device.vnc.port,
            },
            trust::vnc_server_certificates(db, &[id]).await?.remove(&id),
        ),
    })
}

fn reached(message: &'static str) -> ConnectionTest {
    ConnectionTest {
        reachable: true,
        authenticated: false,
        host_key: None,
        host_key_changed: false,
        server_certificate: None,
        certificate_changed: false,
        certificate_matched: false,
        message,
    }
}

/// A refused target is the operator's `Validation` error on `host` (R-032).
fn unreached(
    read: Result<ConnectionTest, ReadError>,
    unexpected: &'static str,
) -> Result<ConnectionTest, Error> {
    let message = match read {
        Ok(test) => return Ok(test),
        Err(ReadError::RefusedTarget) => return Err(Error::Validation(vec![refused_host()])),
        Err(ReadError::Tls) => return Ok(reached(NO_TLS)),
        Err(ReadError::Unresolved) => UNRESOLVED,
        Err(ReadError::Unreachable) => UNREACHABLE,
        Err(ReadError::Timeout) => TIMED_OUT,
        Err(ReadError::Unexpected) => unexpected,
    };
    Ok(ConnectionTest {
        reachable: false,
        ..reached(message)
    })
}

fn rdp_outcome(
    accepted: Option<&ServerCertificate>,
    seen: Result<Seen, ReadError>,
) -> Result<ConnectionTest, Error> {
    let read = seen.map(|seen| match seen {
        Seen::Standard => reached(STANDARD_ONLY),
        Seen::Refused => reached(NEGOTIATION_REFUSED),
        Seen::Certificate(seen) => certificate_outcome(accepted, seen),
    });
    unreached(read, NOT_RDP)
}

fn certificate_outcome(
    accepted: Option<&ServerCertificate>,
    seen: Observation<PresentedCertificate>,
) -> ConnectionTest {
    let seen = ServerCertificate::from(seen);
    if accepted.is_some_and(|accepted| accepted.fingerprint_sha256 == seen.fingerprint_sha256) {
        return ConnectionTest {
            certificate_matched: true,
            ..reached(ACCEPTED_CERTIFICATE)
        };
    }
    ConnectionTest {
        server_certificate: Some(seen),
        certificate_changed: accepted.is_some(),
        ..reached(if accepted.is_some() {
            CHANGED_CERTIFICATE
        } else {
            NEW_CERTIFICATE
        })
    }
}

fn vnc_outcome(
    accepted: Option<&ServerCertificate>,
    read: Result<Option<Observation<PresentedCertificate>>, ReadError>,
) -> Result<ConnectionTest, Error> {
    let read = read.map(|seen| {
        seen.map_or_else(
            || reached(VNC_ANSWERED),
            |seen| certificate_outcome(accepted, seen),
        )
    });
    // A TLS failure is a reachable VNC peer, never a successful identity match.
    if matches!(read, Err(ReadError::Tls)) {
        return Ok(reached(
            "The device answered on VNC but did not complete a supported TLS handshake.",
        ));
    }
    unreached(read, NOT_VNC)
}

#[cfg(test)]
mod tests {
    use chrono::{DateTime, Utc};

    use super::{
        ACCEPTED_CERTIFICATE, CHANGED_CERTIFICATE, NEGOTIATION_REFUSED, NEW_CERTIFICATE, NO_TLS,
        STANDARD_ONLY, Seen, UNREACHABLE, VNC_ANSWERED, rdp_outcome, vnc_outcome,
    };
    use crate::fleet::{
        sessions::connection_test::ReadError,
        trust::{Observation, PresentedCertificate, ServerCertificate},
    };

    fn observed(der: &[u8]) -> Observation<PresentedCertificate> {
        Observation {
            presented: PresentedCertificate::from_der(der),
            first_seen_at: DateTime::<Utc>::UNIX_EPOCH,
            last_seen_at: DateTime::<Utc>::UNIX_EPOCH,
        }
    }

    /// A null `serverCertificate` alone (standard security, a refusal, no TLS) proves nothing.
    #[test]
    fn sec_007_only_the_accepted_certificate_is_matched() {
        let accepted = ServerCertificate::from(observed(b"accepted"));
        let same = || Ok(Seen::Certificate(observed(b"accepted")));
        let (pinned, other) = (Some(&accepted), Ok(Seen::Certificate(observed(b"other"))));
        let cases = [
            ("matched", pinned, same(), ACCEPTED_CERTIFICATE),
            ("first certificate", None, same(), NEW_CERTIFICATE),
            ("changed", pinned, other, CHANGED_CERTIFICATE),
            ("STANDARD_ONLY", pinned, Ok(Seen::Standard), STANDARD_ONLY),
            (
                "NEGOTIATION_REFUSED",
                pinned,
                Ok(Seen::Refused),
                NEGOTIATION_REFUSED,
            ),
            ("NO_TLS", pinned, Err(ReadError::Tls), NO_TLS),
            (
                "unreachable",
                pinned,
                Err(ReadError::Unreachable),
                UNREACHABLE,
            ),
        ];
        for (case, pinned, read, message) in cases {
            let test = rdp_outcome(pinned, read).unwrap_or_else(|_| panic!("{case}"));
            assert_eq!(test.message, message, "{case}");
            assert_eq!(test.certificate_matched, case == "matched", "{case}");
        }
    }

    #[test]
    fn sec_007_a_vnc_banner_without_tls_never_matches_a_certificate() {
        let cases = [
            ("answered", Ok(None)),
            ("unreachable", Err(ReadError::Unreachable)),
        ];
        for (case, read) in cases {
            let test = vnc_outcome(None, read).unwrap_or_else(|_| panic!("{case}"));
            assert!(!test.certificate_matched, "{case}");
        }
        assert_eq!(
            vnc_outcome(None, Ok(None)).map(|test| test.message).ok(),
            Some(VNC_ANSWERED)
        );
    }
}
