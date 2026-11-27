use std::{
    io::{self, Read as _, Write as _},
    path::Path,
    process::{Child, Command, Stdio},
    sync::Arc,
    time::Duration,
};

use rcgen::ExtendedKeyUsagePurpose;
use rustls::pki_types::CertificateDer;
use tokio_rustls::TlsAcceptor;
use zeroize::Zeroizing;

use super::{Issuer, Leaf, invalid, server};

/// Prints only this enrollment's public trust anchor for the holder's TLS verifier.
///
/// # Errors
/// The enrollment or output pipe is unavailable.
pub fn write_public_root(dir: &Path) -> io::Result<()> {
    let issuer = Issuer::at(dir).ok_or_else(invalid)?;
    io::stdout().lock().write_all(issuer.0.der())
}

pub(in crate::threads) fn start(command: &mut Command, dir: &Path) -> io::Result<Child> {
    let identity =
        Issuer::at(dir).and_then(|issuer| issuer.leaf(ExtendedKeyUsagePurpose::ServerAuth));
    let Some(identity) = identity else {
        return command.stdin(Stdio::null()).spawn();
    };
    let bytes = Zeroizing::new(serde_json::to_vec(&identity)?);
    let mut child = command.arg("--channel").stdin(Stdio::piped()).spawn()?;
    if let Err(error) = child.stdin.take().ok_or_else(invalid)?.write_all(&bytes) {
        let _ = child.kill();
        let _ = child.wait();
        return Err(error);
    }
    Ok(child)
}

pub(in crate::threads) async fn public_root(dir: &Path) -> Option<CertificateDer<'static>> {
    let mut command = tokio::process::Command::new(std::env::current_exe().ok()?);
    command
        .arg("holder-public-root")
        .arg(dir)
        .env_clear()
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .kill_on_drop(true);
    let output = tokio::time::timeout(Duration::from_secs(5), command.output())
        .await
        .ok()?
        .ok()?;
    (output.status.success() && output.stdout.len() <= 4096)
        .then(|| CertificateDer::from(output.stdout))
}

pub(in crate::threads) async fn acceptor(
    dir: &Path,
) -> Option<(TlsAcceptor, CertificateDer<'static>)> {
    let bytes = tokio::task::spawn_blocking(|| {
        let mut bytes = Zeroizing::new(Vec::with_capacity(8192));
        io::stdin().lock().take(8192).read_to_end(&mut bytes).ok()?;
        (bytes.len() < 8192).then_some(bytes)
    })
    .await
    .ok()??;
    let identity: Leaf = serde_json::from_slice(&bytes).ok()?;
    let root = public_root(dir).await?;
    Some((
        TlsAcceptor::from(Arc::new(server(&identity, root.clone())?)),
        root,
    ))
}
