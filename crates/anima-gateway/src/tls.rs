//! Desktop TLS verifies handshake signatures and the accepted leaf before credentials are sent.

use std::sync::Arc;

use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::crypto::{CryptoProvider, verify_tls12_signature, verify_tls13_signature};
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use rustls::{ClientConfig, DigitallySignedStruct, SignatureScheme};

#[derive(Debug)]
struct AnyChain(Arc<CryptoProvider>);

impl ServerCertVerifier for AnyChain {
    fn verify_server_cert(
        &self,
        _: &CertificateDer<'_>,
        _: &[CertificateDer<'_>],
        _: &ServerName<'_>,
        _: &[u8],
        _: UnixTime,
    ) -> Result<ServerCertVerified, rustls::Error> {
        Ok(ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        certificate: &CertificateDer<'_>,
        signature: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        let algorithms = &self.0.signature_verification_algorithms;
        verify_tls12_signature(message, certificate, signature, algorithms)
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        certificate: &CertificateDer<'_>,
        signature: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        let algorithms = &self.0.signature_verification_algorithms;
        verify_tls13_signature(message, certificate, signature, algorithms)
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.0.signature_verification_algorithms.supported_schemes()
    }
}

/// No resumed session: the sign-in inside TLS does not allow one (MS-CSSP).
fn any_certificate() -> Result<ClientConfig, rustls::Error> {
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let mut config = ClientConfig::builder_with_provider(Arc::clone(&provider))
        .with_safe_default_protocol_versions()?
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(AnyChain(provider)))
        .with_no_client_auth();
    config.enable_sni = false;
    config.resumption = rustls::client::Resumption::disabled();
    Ok(config)
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum ConnectError {
    #[error("the TLS connection could not be established")]
    Failed,
    #[error("the device certificate is missing or differs from the accepted certificate")]
    Certificate,
}

pub(crate) async fn connect(
    device: tokio::net::TcpStream,
    rule: crate::handshake::Certificate,
) -> Result<tokio_rustls::client::TlsStream<tokio::net::TcpStream>, ConnectError> {
    use sha2::{Digest as _, Sha256};

    let address = device.peer_addr().map_err(|_| ConnectError::Failed)?.ip();
    let config = any_certificate().map_err(|_| ConnectError::Failed)?;
    let connector = tokio_rustls::TlsConnector::from(Arc::new(config));
    let stream = connector
        .connect(ServerName::IpAddress(address.into()), device)
        .await
        .map_err(|_| ConnectError::Failed)?;
    let shown = stream
        .get_ref()
        .1
        .peer_certificates()
        .and_then(<[_]>::first)
        .ok_or(ConnectError::Certificate)?;
    if let crate::handshake::Certificate::Accepted(accepted) = rule
        && Sha256::digest(shown).as_slice() != accepted
    {
        return Err(ConnectError::Certificate);
    }
    Ok(stream)
}
