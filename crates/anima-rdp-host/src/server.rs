use std::{
    fs,
    net::{Ipv4Addr, SocketAddr},
    os::unix::fs::{FileTypeExt, PermissionsExt},
    path::{Path, PathBuf},
    sync::{Arc, mpsc},
    thread,
    time::Duration,
};

use anyhow::{Context as _, Result, bail};
use ironrdp_server::{
    Credentials, DesktopSize, DisplayContext, RdpServer, RdpServerDisplay, RdpServerDisplayUpdates,
    ServerError, ServerErrorExt, ServerResult,
};
use tokio::{
    io::{AsyncRead, AsyncWrite},
    net::{TcpListener, UnixListener},
    sync::oneshot,
};
use tokio_rustls::{TlsAcceptor, rustls};
use zeroize::Zeroizing;

use crate::{
    capture::{Capture, Desktop},
    input::Input,
    permissions,
};

const PORT: u16 = 3389;
const START_LIMIT: Duration = Duration::from_secs(5);
const PERMISSION_INTERVAL: Duration = Duration::from_secs(1);

pub struct Identity {
    pub username: String,
    pub password: Zeroizing<String>,
    pub certificate: Vec<u8>,
    pub private_key: Zeroizing<Vec<u8>>,
    pub public_key: Vec<u8>,
}

pub struct Host {
    stop: Option<oneshot::Sender<()>>,
    thread: Option<thread::JoinHandle<()>>,
}

impl Host {
    /// Start the loopback listener and the dedicated, private Link socket.
    ///
    /// # Errors
    /// The listener or socket cannot be bound, the identity is unusable, or permission is absent.
    pub fn start(path: PathBuf, identity: Identity) -> Result<Self> {
        let (stop, stopped) = oneshot::channel();
        let (ready, started) = mpsc::sync_channel(1);
        let thread = thread::Builder::new()
            .name("anima-rdp-host".into())
            .spawn(move || worker(path, identity, stopped, ready))?;
        let host = Self {
            stop: Some(stop),
            thread: Some(thread),
        };
        started.recv_timeout(START_LIMIT)??;
        Ok(host)
    }

    #[must_use]
    pub fn running(&self) -> bool {
        self.thread
            .as_ref()
            .is_some_and(|thread| !thread.is_finished())
    }
}

impl Drop for Host {
    fn drop(&mut self) {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

type StartReport = mpsc::SyncSender<std::io::Result<()>>;

fn worker(path: PathBuf, identity: Identity, stop: oneshot::Receiver<()>, ready: StartReport) {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build();
    let mut ready = Some(ready);
    if let Err(error) =
        runtime.and_then(|runtime| runtime.block_on(serve(path, identity, stop, &mut ready)))
        && let Some(ready) = ready
    {
        let _ = ready.try_send(Err(error));
    }
}

async fn serve(
    path: PathBuf,
    identity: Identity,
    stop: oneshot::Receiver<()>,
    ready: &mut Option<StartReport>,
) -> std::io::Result<()> {
    let host = Bound::start(path, identity).await?;
    if let Some(ready) = ready.take() {
        let _ = ready.try_send(Ok(()));
    }
    Box::pin(host.run(stop)).await
}

struct Bound {
    tcp: TcpListener,
    unix: UnixListener,
    path: PathBuf,
    address: SocketAddr,
    identity: Identity,
    tls: TlsAcceptor,
}

impl Bound {
    async fn start(path: PathBuf, identity: Identity) -> std::io::Result<Self> {
        Desktop::main().map_err(std::io::Error::other)?;
        let tls = tls(&identity).map_err(std::io::Error::other)?;
        let address = SocketAddr::from((Ipv4Addr::LOCALHOST, PORT));
        let tcp = TcpListener::bind(address).await?;
        let unix = bind_socket(&path).await?;
        Ok(Self {
            tcp,
            unix,
            path,
            address,
            identity,
            tls,
        })
    }

    async fn run(self, mut stopped: oneshot::Receiver<()>) -> std::io::Result<()> {
        let mut tick = tokio::time::interval(PERMISSION_INTERVAL);
        loop {
            tokio::select! {
                biased;
                _ = &mut stopped => return Ok(()),
                () = permission_lost(&mut tick) => return Ok(()),
                accepted = self.unix.accept() => {
                    let (stream, _) = accepted?;
                    if self.session(stream, &mut stopped, &mut tick).await {
                        return Ok(());
                    }
                }
                accepted = self.tcp.accept() => {
                    let (stream, _) = accepted?;
                    if self.session(stream, &mut stopped, &mut tick).await {
                        return Ok(());
                    }
                }
            }
        }
    }

    async fn session<S>(
        &self,
        stream: S,
        stopped: &mut oneshot::Receiver<()>,
        tick: &mut tokio::time::Interval,
    ) -> bool
    where
        S: AsyncRead + AsyncWrite + Send + Sync + Unpin,
    {
        let Ok(desktop) = Desktop::main() else {
            return true;
        };
        let mut server = RdpServer::builder()
            .with_addr(self.address)
            .with_hybrid(self.tls.clone(), self.identity.public_key.clone())
            .with_input_handler(Input::new(desktop))
            .with_display_handler(Display(desktop))
            .build();
        server.set_credentials(Some(Credentials {
            username: self.identity.username.clone(),
            password: self.identity.password.to_string(),
            domain: None,
        }));
        tokio::select! {
            biased;
            _ = stopped => true,
            () = permission_lost(tick) => true,
            _ = server.run_connection(stream) => false,
        }
    }
}

impl Drop for Bound {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

async fn permission_lost(interval: &mut tokio::time::Interval) {
    loop {
        interval.tick().await;
        let found = permissions();
        if !found.screen || !found.input {
            return;
        }
    }
}

struct Display(Desktop);

#[async_trait::async_trait]
impl RdpServerDisplay for Display {
    async fn size(&mut self) -> DesktopSize {
        self.0.size
    }

    async fn updates(
        &mut self,
        _context: DisplayContext,
    ) -> ServerResult<Box<dyn RdpServerDisplayUpdates>> {
        let desktop = self.0;
        let capture = tokio::task::spawn_blocking(move || Capture::start(desktop))
            .await
            .map_err(|_| ServerError::channel("screen capture did not start"))?
            .map_err(|_| ServerError::channel("screen capture did not start"))?;
        Ok(Box::new(capture))
    }
}

async fn bind_socket(path: &Path) -> std::io::Result<UnixListener> {
    if let Ok(metadata) = fs::symlink_metadata(path) {
        if !metadata.file_type().is_socket() {
            return Err(std::io::Error::other("the RDP socket path is occupied"));
        }
        match tokio::time::timeout(
            Duration::from_millis(250),
            tokio::net::UnixStream::connect(path),
        )
        .await
        {
            Ok(Err(error)) if error.kind() == std::io::ErrorKind::ConnectionRefused => {
                fs::remove_file(path)?;
            }
            _ => return Err(std::io::Error::other("the RDP socket is in use")),
        }
    }
    let listener = UnixListener::bind(path)?;
    if let Err(error) = fs::set_permissions(path, fs::Permissions::from_mode(0o600)) {
        let _ = fs::remove_file(path);
        return Err(error);
    }
    Ok(listener)
}

fn tls(identity: &Identity) -> Result<TlsAcceptor> {
    if identity.password.is_empty() || identity.username.is_empty() {
        bail!("RDP requires a dedicated credential");
    }
    let config = rustls::ServerConfig::builder_with_provider(Arc::new(
        rustls::crypto::ring::default_provider(),
    ))
    .with_safe_default_protocol_versions()?
    .with_no_client_auth()
    .with_single_cert(
        vec![rustls::pki_types::CertificateDer::from(
            identity.certificate.clone(),
        )],
        rustls::pki_types::PrivateKeyDer::try_from(identity.private_key.to_vec())
            .map_err(anyhow::Error::msg)
            .context("invalid RDP private key")?,
    )?;
    Ok(TlsAcceptor::from(Arc::new(config)))
}
