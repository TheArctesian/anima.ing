//! Host keys and RDP certificates that connection tests saw, kept 15 minutes (SEC-006, SEC-007).

use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use chrono::{DateTime, Utc};

use super::{
    CertificateProtocol,
    certificate::PresentedCertificate,
    observations::{Endpoint, Observation, Observations},
    public_key::PublicKey,
};

#[derive(Clone, Default)]
pub struct Sightings {
    /// Its own lock: a connection test never waits on a ticket redemption.
    observations: Arc<Mutex<Observations<PublicKey>>>,
    certificates: Arc<Mutex<[Observations<PresentedCertificate>; 2]>>,
}

impl Sightings {
    fn observations(&self) -> MutexGuard<'_, Observations<PublicKey>> {
        self.observations
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }

    fn certificates(&self) -> MutexGuard<'_, [Observations<PresentedCertificate>; 2]> {
        self.certificates
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }

    pub(crate) fn observe(
        &self,
        endpoint: Endpoint,
        key: PublicKey,
        now: DateTime<Utc>,
    ) -> Observation<PublicKey> {
        self.observations().record(endpoint, key, now)
    }

    pub(in crate::fleet::trust) fn observed(
        &self,
        endpoint: &Endpoint,
        now: DateTime<Utc>,
    ) -> Option<Observation<PublicKey>> {
        self.observations().find(endpoint, now)
    }

    pub(crate) fn observe_certificate(
        &self,
        (endpoint, protocol): (Endpoint, CertificateProtocol),
        certificate: PresentedCertificate,
        now: DateTime<Utc>,
    ) -> Observation<PresentedCertificate> {
        self.certificates()[protocol.index()].record(endpoint, certificate, now)
    }

    pub(in crate::fleet::trust) fn observed_certificate(
        &self,
        (endpoint, protocol): (&Endpoint, CertificateProtocol),
        now: DateTime<Utc>,
    ) -> Option<Observation<PresentedCertificate>> {
        self.certificates()[protocol.index()].find(endpoint, now)
    }
}
