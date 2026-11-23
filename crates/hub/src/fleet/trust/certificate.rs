//! Only the SHA-256 of an RDP leaf certificate's DER is pinned; its subject and expiry are
//! shown and never trusted (SEC-007). Pure.

use chrono::{DateTime, Utc};

use super::{model::ServerCertificate, observations::Observation};
use crate::fingerprint::CertificateFingerprint;

/// In characters, as `server_certificates_subject_check` allows.
pub(in crate::fleet::trust) const MAX_SUBJECT: usize = 1024;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct PresentedCertificate {
    pub(crate) fingerprint: CertificateFingerprint,
    pub(crate) subject: Option<String>,
    pub(crate) not_after: Option<DateTime<Utc>>,
}

impl PresentedCertificate {
    pub(crate) fn from_der(der: &[u8]) -> Self {
        let parsed = x509_parser::parse_x509_certificate(der).ok();
        let subject = parsed
            .as_ref()
            .and_then(|(_, cert)| cleaned_subject(&cert.subject().to_string()));
        let not_after = parsed.as_ref().and_then(|(_, cert)| {
            DateTime::from_timestamp(cert.validity().not_after.timestamp(), 0)
        });
        Self {
            fingerprint: CertificateFingerprint::of(der),
            subject,
            not_after,
        }
    }
}

impl From<Observation<PresentedCertificate>> for ServerCertificate {
    fn from(seen: Observation<PresentedCertificate>) -> Self {
        Self {
            fingerprint_sha256: seen.presented.fingerprint.colon_pairs(),
            subject: seen.presented.subject,
            not_after: seen.presented.not_after,
            first_seen_at: seen.first_seen_at,
        }
    }
}

/// The device chooses the subject, so it must not break a line, hide text or reorder it
/// wherever it is shown.
pub(in crate::fleet::trust) fn cleaned_subject(text: &str) -> Option<String> {
    let kept = crate::text::clean(text, usize::MAX);
    let kept: String = kept.trim().chars().take(MAX_SUBJECT).collect();
    let kept = kept.trim_end();
    (!kept.is_empty()).then(|| kept.to_owned())
}
