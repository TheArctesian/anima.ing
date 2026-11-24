//! The relay between an agent program of a thread and its model provider (task SA6, Section 7
//! of `docs/proposals/shortened-answers.md`). This version is pass-through only: each request
//! goes to the provider unchanged and each answer comes back unchanged, as a stream.
//!
//! It listens on the loopback address only. A request is forwarded only when its path starts
//! with the secret of a live thread, and only to the provider of that thread's program: the
//! hosts are fixed in [`upstream`], and nothing in a request can name a different one. A
//! request with an `Origin` header (a web page) is refused. No redirect is followed.
//!
//! Sign-in data (`authorization`, `x-api-key`, cookies) passes through in memory. This crate
//! never writes a header value, a path or a body to a log or a file: a path holds the secret.
//! The thread holder owns the listener and publishes only its public CA to children.

mod forward;
mod headers;
mod secrets;
mod serve;
#[cfg(test)]
mod tests;
mod tls;
pub mod upstream;

use std::io;
use std::net::{Ipv4Addr, SocketAddr};
use std::sync::Arc;

use tokio::net::TcpListener;
use tokio::task::JoinHandle;

pub use secrets::SecretError;
use secrets::Secrets;
use upstream::{Program, Upstreams};

/// The relay while it listens. When it is dropped, it stops.
pub struct Relay {
    address: SocketAddr,
    secrets: Arc<Secrets>,
    listening: JoinHandle<()>,
    certificate: String,
}

impl Relay {
    /// Listens on a free port of `127.0.0.1` and forwards to the providers.
    ///
    /// # Errors
    /// The error of the system when no port can be bound.
    pub async fn start() -> io::Result<Self> {
        Self::listen(Upstreams::providers(), Some(tls::Identity::generate()?)).await
    }

    #[cfg(test)]
    pub(crate) async fn start_with(upstreams: Upstreams) -> io::Result<Self> {
        Self::listen(upstreams, None).await
    }

    async fn listen(upstreams: Upstreams, identity: Option<tls::Identity>) -> io::Result<Self> {
        let (certificate, acceptor) = identity.map_or_else(
            || (String::new(), None),
            |identity| (identity.certificate, Some(identity.acceptor)),
        );
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await?;
        let address = listener.local_addr()?;
        let secrets = Arc::new(Secrets::default());
        let listening = tokio::spawn(serve::accept(
            listener,
            secrets.clone(),
            Arc::new(upstreams),
            acceptor,
        ));
        Ok(Self {
            address,
            secrets,
            listening,
            certificate,
        })
    }

    /// Opens one thread and gives its private route material. Claude uses this address;
    /// the holder moves the Codex path secret into its private authentication header before
    /// configuring the child. This value must never enter runner records or arguments.
    ///
    /// # Errors
    /// [`SecretError`] when the name is not a name, the thread is open already or the system
    /// gives no random bytes.
    pub fn open(&self, thread: &str, program: Program) -> Result<String, SecretError> {
        let secret = self.secrets.open(thread, program)?;
        Ok(format!(
            "{}://{}/{secret}{}",
            if self.certificate.is_empty() {
                "http"
            } else {
                "https"
            },
            self.address,
            program.base_path()
        ))
    }

    /// Public trust material for this holder. Contains no private key or thread secret.
    #[must_use]
    pub fn certificate(&self) -> &str {
        &self.certificate
    }

    /// Ends the secret of a thread. A request with it gets 404 from now on. An answer that is
    /// on its way goes on to its end; the runner ends the program of the thread.
    pub fn close(&self, thread: &str) {
        self.secrets.close(thread);
    }
}

impl Drop for Relay {
    fn drop(&mut self) {
        // The connections are tasks of the listener: they stop with it.
        self.listening.abort();
        self.secrets.close_all();
    }
}
