//! Mutual TLS on the holder socket. Only the daemon role issues identities. A holder receives
//! its leaf key through the pipe to the process the daemon starts; neither role sends a leaf
//! private key over the named socket. The holder obtains the public enrollment root separately.

use std::{io, path::Path, sync::Arc};

#[cfg(any(target_os = "macos", test))]
use hmac::{Hmac, KeyInit as _, Mac as _};
#[cfg(any(target_os = "macos", test))]
use rcgen::{BasicConstraints, DnType, IsCa};
use rcgen::{
    CertificateParams, CertifiedIssuer, ExtendedKeyUsagePurpose, KeyPair, KeyUsagePurpose,
    PKCS_ED25519,
};
use rustls::{
    ClientConfig, RootCertStore, ServerConfig,
    pki_types::{CertificateDer, PrivatePkcs8KeyDer},
};
use serde::{Deserialize, Serialize};
#[cfg(any(target_os = "macos", test))]
use sha2::Sha256;
use zeroize::Zeroize as _;
#[cfg(any(target_os = "macos", test))]
use zeroize::Zeroizing;

#[cfg(any(target_os = "macos", test))]
use crate::config::Config;

pub(super) const SOCKET: &str = "hold.tls";
pub(super) const NAME: &str = "holder.anima.invalid";
mod bootstrap;
pub use bootstrap::write_public_root;
pub(super) use bootstrap::{acceptor, public_root, start};

pub(crate) struct Issuer(CertifiedIssuer<'static, KeyPair>);

#[derive(Serialize, Deserialize)]
struct Leaf {
    certificate: Vec<u8>,
    key: Vec<u8>,
}

impl Drop for Leaf {
    fn drop(&mut self) {
        self.key.zeroize();
    }
}

impl Issuer {
    #[cfg(any(target_os = "macos", test))]
    fn for_device(key: &[u8; 32], config: &Config, home: &Path) -> Option<Self> {
        // RFC 8410's PKCS#8 OneAsymmetricKey encoding for a 32-byte Ed25519 seed.
        const ED25519: &[u8] = &[
            0x30, 0x2e, 2, 1, 0, 0x30, 5, 6, 3, 0x2b, 0x65, 0x70, 4, 0x22, 4, 0x20,
        ];
        let mut mac = Hmac::<Sha256>::new_from_slice(key).ok()?;
        mac.update(b"anima-holder-tls-v1\0");
        mac.update(config.device.as_bytes());
        mac.update(config.hub.as_text().as_bytes());
        mac.update(b"\0");
        mac.update(home.canonicalize().ok()?.as_os_str().as_encoded_bytes());
        let mut key = Zeroizing::new(Vec::with_capacity(ED25519.len() + 32));
        key.extend_from_slice(ED25519);
        key.extend_from_slice(&mac.finalize().into_bytes());
        let der = PrivatePkcs8KeyDer::from(key.as_slice());
        let issuer_key = KeyPair::from_pkcs8_der_and_sign_algo(&der, &PKCS_ED25519).ok()?;
        let mut params = CertificateParams::default();
        params
            .distinguished_name
            .push(DnType::CommonName, "Anima holder channel v1");
        params.is_ca = IsCa::Ca(BasicConstraints::Constrained(0));
        params.key_usages = vec![KeyUsagePurpose::KeyCertSign];
        params.serial_number = Some(1_u64.into());
        Some(Self(CertifiedIssuer::self_signed(params, issuer_key).ok()?))
    }

    #[cfg(target_os = "macos")]
    pub(super) fn at(dir: &Path) -> Option<Self> {
        use crate::{config::home::Home, token_store::TokenStore};
        let home = Home::at(dir.parent()?.to_owned());
        let config = home.read_config().ok()??;
        let key = TokenStore::open(&home).holder_authority(&config.hub)?;
        Self::for_device(&key, &config, home.dir())
    }

    #[cfg(not(target_os = "macos"))]
    pub(super) fn at(_: &Path) -> Option<Self> {
        None
    }

    fn leaf(&self, role: ExtendedKeyUsagePurpose) -> Option<Leaf> {
        let mut params = CertificateParams::new(vec![NAME.to_owned()]).ok()?;
        params.extended_key_usages = vec![role];
        params.key_usages = vec![KeyUsagePurpose::DigitalSignature];
        let key = KeyPair::generate_for(&PKCS_ED25519).ok()?;
        let signed = params.signed_by(&key, &self.0).ok()?;
        Some(Leaf {
            certificate: signed.der().to_vec(),
            key: key.serialize_der(),
        })
    }

    pub(super) fn root(&self) -> CertificateDer<'static> {
        self.0.der().clone()
    }

    pub(super) fn client(&self) -> Option<ClientConfig> {
        let leaf = self.leaf(ExtendedKeyUsagePurpose::ClientAuth)?;
        ClientConfig::builder_with_provider(provider())
            .with_protocol_versions(&[&rustls::version::TLS13])
            .ok()?
            .with_root_certificates(roots(self.0.der().clone())?)
            .with_client_auth_cert(
                vec![CertificateDer::from(leaf.certificate.clone())],
                PrivatePkcs8KeyDer::from(leaf.key.clone()).into(),
            )
            .ok()
    }
}

pub(super) async fn available(dir: &Path) -> bool {
    let dir = dir.to_owned();
    tokio::task::spawn_blocking(move || Issuer::at(&dir).is_some())
        .await
        .unwrap_or(false)
}

fn provider() -> Arc<rustls::crypto::CryptoProvider> {
    Arc::new(rustls::crypto::ring::default_provider())
}

fn roots(cert: CertificateDer<'static>) -> Option<RootCertStore> {
    let mut roots = RootCertStore::empty();
    roots.add(cert).ok()?;
    Some(roots)
}

fn server(leaf: &Leaf, root: CertificateDer<'static>) -> Option<ServerConfig> {
    let verifier = rustls::server::WebPkiClientVerifier::builder_with_provider(
        Arc::new(roots(root)?),
        provider(),
    )
    .build()
    .ok()?;
    ServerConfig::builder_with_provider(provider())
        .with_protocol_versions(&[&rustls::version::TLS13])
        .ok()?
        .with_client_cert_verifier(verifier)
        .with_single_cert(
            vec![CertificateDer::from(leaf.certificate.clone())],
            PrivatePkcs8KeyDer::from(leaf.key.clone()).into(),
        )
        .ok()
}

fn invalid() -> io::Error {
    io::Error::new(
        io::ErrorKind::PermissionDenied,
        "holder channel credential unavailable",
    )
}

#[cfg(test)]
mod tests;
