use std::{
    io::{self, Read as _, Write as _},
    net::Shutdown,
    path::Path,
    process::{Child, Command, Stdio},
    sync::Arc,
};

use rcgen::ExtendedKeyUsagePurpose;
use rustls::pki_types::CertificateDer;
use tokio_rustls::TlsAcceptor;
use zeroize::Zeroizing;

use super::{Issuer, Leaf, invalid, server};
#[cfg(target_os = "macos")]
mod peer;
mod rendezvous;

/// Returns this enrollment's public trust anchor on the bootstrap connection.
///
/// # Errors
/// The enrollment or output pipe is unavailable.
pub fn write_public_root(dir: &Path) -> io::Result<()> {
    let issuer = Issuer::at(dir).ok_or_else(invalid)?;
    let mut socket = rendezvous::connect()?;
    socket.write_all(issuer.0.der())?;
    socket.shutdown(Shutdown::Write)?;
    let mut ack = [0];
    socket.read_exact(&mut ack)
}

pub(in crate::threads) fn start(command: &mut Command, dir: &Path) -> io::Result<Child> {
    let identity = Issuer::transport(dir)?
        .map(|issuer| {
            issuer
                .leaf(ExtendedKeyUsagePurpose::ServerAuth)
                .ok_or_else(invalid)
        })
        .transpose()?;
    let Some(identity) = identity else {
        return command.stdin(Stdio::null()).spawn();
    };
    let bytes = Zeroizing::new(serde_json::to_vec(&identity)?);
    let (child, ()) = rendezvous::with_child(command.arg("--channel"), |mut socket| {
        socket.write_all(&bytes)
    })?;
    Ok(child)
}

pub(in crate::threads) async fn public_root(dir: &Path) -> Option<CertificateDer<'static>> {
    let dir = dir.to_owned();
    tokio::task::spawn_blocking(move || {
        let mut command = Command::new(std::env::current_exe().ok()?);
        command
            .arg("holder-public-root")
            .arg(dir)
            .env_clear()
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        let (mut child, root) = rendezvous::with_child(&mut command, |mut socket| {
            let mut bytes = Vec::with_capacity(4096);
            (&socket).take(4097).read_to_end(&mut bytes)?;
            if bytes.is_empty() || bytes.len() > 4096 {
                return Err(invalid());
            }
            socket.write_all(&[1])?;
            Ok(CertificateDer::from(bytes))
        })
        .ok()?;
        let until = std::time::Instant::now() + std::time::Duration::from_secs(2);
        while std::time::Instant::now() < until {
            if let Some(status) = child.try_wait().ok()? {
                return status.success().then_some(root);
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        let _ = child.kill();
        let _ = child.wait();
        None
    })
    .await
    .ok()?
}

pub(in crate::threads) async fn acceptor(
    dir: &Path,
) -> Option<(TlsAcceptor, CertificateDer<'static>)> {
    let bytes = tokio::task::spawn_blocking(|| {
        let mut bytes = Zeroizing::new(Vec::with_capacity(8192));
        rendezvous::connect()
            .ok()?
            .take(8192)
            .read_to_end(&mut bytes)
            .ok()?;
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
