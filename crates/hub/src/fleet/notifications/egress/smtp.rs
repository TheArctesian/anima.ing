//! SMTP egress (D-17): one message to a mail server at its checked addresses, encrypted before
//! any command that carries a credential or the message; there is no plaintext mode.
//!
//! SNI and the certificate check use the host name, not the checked address.
//! The reply after DATA decides the outcome and the answer to QUIT is ignored, so a server that
//! queued the message is not sent to again (S-A).
//! The client greets with `[127.0.0.1]` (RFC 5321 4.1.4), so no mail server learns the hub's host name.

use std::{
    io,
    net::{Ipv4Addr, SocketAddr},
};

use lettre::{
    Message,
    transport::smtp::{
        self,
        authentication::{Credentials, Mechanism},
        client::{AsyncSmtpConnection, Certificate, CertificateStore, TlsParameters},
        extension::ClientId,
    },
};

use serde::{Deserialize, Serialize};
use tokio::time::{Instant, timeout_at};

use super::{Egress, SendError, TIME_LIMIT, causes, tls_broke};
use crate::fleet::targets::resolve_checked;

/// An e-mail channel stores it as `tls` or `starttls` (K1 `SmtpSecurity`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum SmtpSecurity {
    /// TLS from the first byte (port 465).
    #[serde(rename = "tls")]
    Implicit,
    /// STARTTLS before anything else (port 587). Implicit TLS is preferred: lettre reads a reply
    /// injected in plaintext right after the STARTTLS answer as if it came over TLS.
    #[serde(rename = "starttls")]
    StartTls,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SmtpServer {
    /// A host name or IP literal; resolved and checked on each send.
    pub(crate) host: String,
    pub(crate) port: u16,
    pub(crate) security: SmtpSecurity,
}

const MECHANISMS: [Mechanism; 2] = [Mechanism::Plain, Mechanism::Login];

pub(super) const HELLO: ClientId = ClientId::Ipv4(Ipv4Addr::LOCALHOST);

impl Egress {
    /// The outcome is decided within [`TIME_LIMIT`] from the lookup (S-A).
    ///
    /// # Errors
    /// [`SendError::Target`] with no connection made for a refused or unresolved host; else the
    /// [`SendError`] of the failed step.
    pub(crate) async fn send_mail(
        &self,
        server: &SmtpServer,
        login: Option<&Credentials>,
        message: &Message,
    ) -> Result<(), SendError> {
        let deadline = Instant::now() + TIME_LIMIT;
        let (mut connection, outcome) = timeout_at(deadline, self.deliver(server, login, message))
            .await
            .map_err(|_| SendError::TimedOut)??;
        let _ = timeout_at(deadline, connection.abort()).await;
        outcome
    }

    async fn deliver(
        &self,
        server: &SmtpServer,
        login: Option<&Credentials>,
        message: &Message,
    ) -> Result<(AsyncSmtpConnection, Result<(), SendError>), SendError> {
        let addresses = resolve_checked(&server.host, server.port, &self.policy).await?;
        let tls = self.tls_parameters(&server.host)?;
        let mut connection = open(&addresses, server.security, tls).await?;
        let outcome = submit(&mut connection, login, message).await;
        Ok((connection, outcome))
    }

    pub(super) fn tls_parameters(&self, host: &str) -> Result<TlsParameters, SendError> {
        let name = host
            .strip_prefix('[')
            .and_then(|inside| inside.strip_suffix(']'))
            .unwrap_or(host);
        let mut builder = TlsParameters::builder(name.to_owned())
            .certificate_store(CertificateStore::WebpkiRoots);
        for der in &self.extra_roots {
            let root = Certificate::from_der(der.clone()).map_err(|_| SendError::Setup)?;
            builder = builder.add_root_certificate(root);
        }
        builder.build_rustls().map_err(|_| SendError::Setup)
    }
}

/// Never returns a connection that is not encrypted.
pub(super) async fn open(
    addresses: &[SocketAddr],
    security: SmtpSecurity,
    tls: TlsParameters,
) -> Result<AsyncSmtpConnection, SendError> {
    let wrapper = (security == SmtpSecurity::Implicit).then(|| tls.clone());
    let mut connection =
        AsyncSmtpConnection::connect_tokio1(addresses, Some(TIME_LIMIT), &HELLO, wrapper, None)
            .await
            .map_err(|error| classify(&error))?;
    if security == SmtpSecurity::StartTls {
        if !connection.can_starttls() {
            connection.abort().await;
            return Err(SendError::NoStartTls);
        }
        connection
            .starttls(tls, &HELLO)
            .await
            .map_err(|error| classify(&error))?;
    }
    if connection.is_encrypted() {
        Ok(connection)
    } else {
        connection.abort().await;
        Err(SendError::NoStartTls)
    }
}

async fn submit(
    connection: &mut AsyncSmtpConnection,
    login: Option<&Credentials>,
    message: &Message,
) -> Result<(), SendError> {
    if let Some(login) = login {
        connection
            .auth(&MECHANISMS, login)
            .await
            .map_err(|error| classify(&error))?;
    }
    connection
        .send(message.envelope(), &message.formatted())
        .await
        .map_err(|error| classify(&error))?;
    Ok(())
}

/// Judged from the error's kind and causes, never its text.
fn classify(error: &smtp::Error) -> SendError {
    if let Some(code) = error.status() {
        SendError::SmtpReply(u16::from(code))
    } else if error.is_timeout() {
        SendError::TimedOut
    } else if error.is_tls() || tls_broke(error) {
        SendError::Tls
    } else if causes(error).any(<dyn std::error::Error>::is::<io::Error>) {
        SendError::Connect
    } else {
        SendError::Exchange
    }
}
