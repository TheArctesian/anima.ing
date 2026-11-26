//! The connection test (DEV-013, SEC-006, SEC-007). The host key or certificate it sees is
//! recorded in `trust::Sightings` for `acceptHostKey` and `acceptServerCertificate`.

mod desktop_test;
mod rdp;
mod reach;
mod ssh_test;
#[cfg(test)]
mod tests;
mod tls;
mod vnc;

pub(super) use desktop_test::{DesktopSettings, DesktopSubject};
pub(super) use rdp::{Reply, read_server_certificate};
pub(super) use reach::ReadError;
use remote_login::x224;
pub(super) use remote_login::x224::TLS_PROTOCOLS;
pub(super) use ssh_test::{ConnectionTest, TestRequest, TestSubject, Unsaved, test_connection};
pub(super) use vnc::read_certificate as read_vnc_certificate;
