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
//! Nothing here starts the relay; the daemon has no caller yet.

mod forward;
mod headers;
mod secrets;
mod serve;
#[cfg(test)]
mod tests;
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
}

impl Relay {
    /// Listens on a free port of `127.0.0.1` and forwards to the providers.
    ///
    /// # Errors
    /// The error of the system when no port can be bound.
    pub async fn start() -> io::Result<Self> {
        Self::start_with(Upstreams::providers()).await
    }

    pub(crate) async fn start_with(upstreams: Upstreams) -> io::Result<Self> {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await?;
        let address = listener.local_addr()?;
        let secrets = Arc::new(Secrets::default());
        let listening = tokio::spawn(serve::accept(
            listener,
            secrets.clone(),
            Arc::new(upstreams),
        ));
        Ok(Self {
            address,
            secrets,
            listening,
        })
    }

    /// Opens the relay for one thread and gives the base address for its program: the value
    /// of `ANTHROPIC_BASE_URL` for Claude Code, of `openai_base_url` for Codex. The address
    /// holds the secret of the thread, so it must reach only that program.
    ///
    /// # Errors
    /// [`SecretError`] when the name is not a name, the thread is open already or the system
    /// gives no random bytes.
    pub fn open(&self, thread: &str, program: Program) -> Result<String, SecretError> {
        let secret = self.secrets.open(thread, program)?;
        Ok(format!(
            "http://{}/{secret}{}",
            self.address,
            program.base_path()
        ))
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
