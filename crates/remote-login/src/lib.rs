//! Owner-initiated SSH setup and credential-free remote protocol negotiation.

pub mod desktop;
mod elevation;
mod probe;
mod vnc;
pub mod x224;

pub use probe::{LocalProtocol, Observation, observe};

use std::time::Duration;

#[derive(Clone, Copy)]
pub enum Prompt {
    Desktop,
    Terminal,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(
        "Use a non-root account with a user name containing only letters, numbers, dots, hyphens or underscores."
    )]
    User,
    #[error("Automatic SSH setup is not available on this operating system.")]
    Unsupported,
    #[error(
        "VNC authentication could not be verified. Configure mandatory VNC password or Apple account authentication, then try again. Screen-login modes such as x11vnc unixpw cannot be verified automatically. No private port was opened."
    )]
    VncAuthentication,
    #[error(
        "Administrator authorization was cancelled or SSH setup failed. No private port was opened."
    )]
    Setup,
    #[error("SSH setup is still waiting or taking too long. Check again before retrying.")]
    TimedOut,
    #[error("SSH did not become ready. No private port was opened.")]
    NotReady,
}

/// Installs or starts a loopback SSH service, preserving an already working SSH server.
/// The operating system asks for elevation; Anima never handles the administrator password.
///
/// # Errors
/// Returns a fixed, non-secret failure when setup or its SSH banner check fails.
pub async fn enable(prompt: Prompt) -> Result<String, Error> {
    let user = whoami::username().map_err(|_| Error::User)?;
    if !valid_user(&user) {
        return Err(Error::User);
    }
    let existing = probe::ready().await;
    elevation::install(prompt, &user, existing).await?;
    for _ in 0..30 {
        if probe::ready().await {
            return Ok(user);
        }
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
    Err(Error::NotReady)
}

fn valid_user(user: &str) -> bool {
    !user.is_empty()
        && user != "root"
        && user.len() <= 64
        && user
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"_-.".contains(&c))
}

/// Whether the local SSH server currently answers with an SSH banner.
pub async fn ready() -> bool {
    probe::ready().await
}
