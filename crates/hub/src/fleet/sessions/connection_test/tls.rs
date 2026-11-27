//! Reads a TLS peer's leaf certificate without sending account credentials.
//! Handshake signatures are checked; trust is decided separately by fingerprint.

use std::{net::IpAddr, sync::Arc};

use rustls::{
    ClientConfig, DigitallySignedStruct, SignatureScheme,
    client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier},
    crypto::{CryptoProvider, ring, verify_tls12_signature, verify_tls13_signature},
    pki_types::{CertificateDer, ServerName, UnixTime},
};
use tokio_rustls::TlsConnector;

use super::reach::ReadError;
use crate::fleet::{targets::Reached, trust::PresentedCertificate};

pub(super) async fn leaf(stream: Reached) -> Result<PresentedCertificate, ReadError> {
    // Link sockets have no IP peer. The placeholder is never sent as SNI or used as trust.
    let peer = match &stream {
        Reached::Tcp(_) => stream.network_peer().ok_or(ReadError::Unreachable)?,
        Reached::Link(_) => IpAddr::from([0, 0, 0, 0]),
    };
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
