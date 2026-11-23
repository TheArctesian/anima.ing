//! Reading an RDP server's certificate (SEC-007, K4 §3). No credential is sent: the hub never starts `CredSSP`.
//! Nothing here logs, and no error holds a remote text or a host name (LOG-007).
//! The verifier accepts any certificate, since the operator decides by fingerprint, but checks the handshake signatures.

use std::{net::IpAddr, sync::Arc, time::Duration};

use rustls::{
    ClientConfig, DigitallySignedStruct, SignatureScheme,
    client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier},
    crypto::{CryptoProvider, ring, verify_tls12_signature, verify_tls13_signature},
    pki_types::{CertificateDer, ServerName, UnixTime},
};
use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};
use tokio_rustls::TlsConnector;

use super::{
    reach::{ReadError, reach, within},
    x224::{self, Negotiated, PROTOCOL_HYBRID, PROTOCOL_HYBRID_EX, TPKT_HEADER},
};
use crate::fleet::{
    devices::RdpSecurity, ssh::Target, targets::Reached, trust::PresentedCertificate,
};

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
        match confirm(&mut stream, requested).await? {
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

async fn confirm(stream: &mut Reached, requested: u32) -> Result<Negotiated, ReadError> {
    let unexpected = |_| ReadError::Unexpected;
    stream
        .write_all(&x224::connection_request(requested))
        .await
        .map_err(unexpected)?;
    let mut header = [0; TPKT_HEADER];
    stream.read_exact(&mut header).await.map_err(unexpected)?;
    let mut body = vec![0; x224::confirm_length(header).map_err(|_| ReadError::Unexpected)?];
    stream.read_exact(&mut body).await.map_err(unexpected)?;
    x224::negotiated(&body, requested).map_err(|_| ReadError::Unexpected)
}

async fn leaf(stream: Reached) -> Result<PresentedCertificate, ReadError> {
    // A link computer has no network address to name: RDP through the link is HM-T14.
    let peer = stream.network_peer().ok_or(ReadError::Unreachable)?;
    let tls = connector()?
        .connect(server_name(peer), stream)
        .await
        .map_err(|_| ReadError::Tls)?;
    let (_, session) = tls.get_ref();
    let der = session
        .peer_certificates()
        .and_then(<[_]>::first)
        .ok_or(ReadError::Tls)?;
    Ok(PresentedCertificate::from_der(der))
}

/// rustls sends no SNI for an IP address, and the verifier does not judge the name.
fn server_name(peer: IpAddr) -> ServerName<'static> {
    ServerName::IpAddress(peer.into())
}

fn connector() -> Result<TlsConnector, ReadError> {
    let provider = Arc::new(ring::default_provider());
    let verifier = Arc::new(AnyCertificate(provider.clone()));
    let mut config = ClientConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .map_err(|_| ReadError::Tls)?
        .dangerous()
        .with_custom_certificate_verifier(verifier)
        .with_no_client_auth();
    config.enable_sni = false;
    Ok(TlsConnector::from(Arc::new(config)))
}

#[derive(Debug)]
struct AnyCertificate(Arc<CryptoProvider>);

impl ServerCertVerifier for AnyCertificate {
    fn verify_server_cert(
        &self,
        _end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: UnixTime,
    ) -> Result<ServerCertVerified, rustls::Error> {
        Ok(ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        verify_tls12_signature(
            message,
            cert,
            dss,
            &self.0.signature_verification_algorithms,
        )
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        verify_tls13_signature(
            message,
            cert,
            dss,
            &self.0.signature_verification_algorithms,
        )
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.0.signature_verification_algorithms.supported_schemes()
    }
}
