//! A fresh holder identity: public trust material, private keys only in memory.

use std::io;
use std::sync::Arc;

use rcgen::{
    BasicConstraints, CertificateParams, ExtendedKeyUsagePurpose, IsCa, Issuer, KeyPair,
    KeyUsagePurpose,
};
use rustls::pki_types::{PrivateKeyDer, PrivatePkcs8KeyDer};
use tokio_rustls::TlsAcceptor;

pub(crate) struct Identity {
    pub certificate: String,
    pub acceptor: TlsAcceptor,
}

impl Identity {
    pub fn generate() -> io::Result<Self> {
        let mut ca = CertificateParams::default();
        ca.is_ca = IsCa::Ca(BasicConstraints::Constrained(0));
        ca.key_usages = vec![KeyUsagePurpose::KeyCertSign];
        let ca_key = KeyPair::generate().map_err(io::Error::other)?;
        let certificate = ca.self_signed(&ca_key).map_err(io::Error::other)?.pem();
        let issuer = Issuer::new(ca, ca_key);
        let mut leaf =
            CertificateParams::new(vec!["127.0.0.1".to_owned()]).map_err(io::Error::other)?;
        leaf.extended_key_usages = vec![ExtendedKeyUsagePurpose::ServerAuth];
        let key = KeyPair::generate().map_err(io::Error::other)?;
        let leaf = leaf.signed_by(&key, &issuer).map_err(io::Error::other)?;
        let config = rustls::ServerConfig::builder_with_provider(Arc::new(
            rustls::crypto::ring::default_provider(),
        ))
        .with_safe_default_protocol_versions()
        .map_err(io::Error::other)?
        .with_no_client_auth()
        .with_single_cert(
            vec![leaf.der().clone()],
            PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(key.serialize_der())),
        )
        .map_err(io::Error::other)?;
        Ok(Self {
            certificate,
            acceptor: TlsAcceptor::from(Arc::new(config)),
        })
    }
}
