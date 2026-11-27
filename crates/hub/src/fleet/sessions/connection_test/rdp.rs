//! Reading an RDP server's certificate (SEC-007, K4 §3). No credential is sent: the hub never starts `CredSSP`.
//! Nothing here logs, and no error holds a remote text or a host name (LOG-007).
//! The verifier accepts any certificate, since the operator decides by fingerprint, but checks the handshake signatures.

use std::time::Duration;

use super::{
    reach::{ReadError, reach, within},
    tls::leaf,
    x224::{self, Negotiated, PROTOCOL_HYBRID, PROTOCOL_HYBRID_EX},
};
use crate::fleet::{devices::RdpSecurity, ssh::Target, trust::PresentedCertificate};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::fleet::sessions) enum Reply {
    Certificate(PresentedCertificate, RdpSecurity),
    Standard,
    Refused,
}

/// `requested` is a set of `x224::PROTOCOL_*` flags.
pub(in crate::fleet::sessions) async fn read_server_certificate(
    target: Target<'_>,
    requested: u32,
    limit: Duration,
) -> Result<Reply, ReadError> {
    within(limit, async {
        let mut stream = reach(target).await?;
        match x224::negotiate(&mut stream, requested)
            .await
            .map_err(|_| ReadError::Unexpected)?
        {
            Negotiated::Standard => Ok(Reply::Standard),
            Negotiated::Failure(_) => Ok(Reply::Refused),
            Negotiated::Tls(chosen) => {
                let certificate = leaf(stream).await?;
                Ok(Reply::Certificate(certificate, chosen_security(chosen)))
            }
        }
    })
    .await
}

const fn chosen_security(chosen: u32) -> RdpSecurity {
    if chosen & PROTOCOL_HYBRID_EX != 0 {
        RdpSecurity::NlaExt
    } else if chosen & PROTOCOL_HYBRID != 0 {
        RdpSecurity::Nla
    } else {
        RdpSecurity::Tls
    }
}
