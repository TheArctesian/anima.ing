//! Moves an enrolment with the old link origin to the link socket once its peer passes the check (R-172, R-317).
//! After the move the path is kept with the token, so no value of the environment decides it again.

use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::config::Config;
use crate::config::home::{Home, HomeError};
use crate::config::origin::{Origin, Reach};
use crate::hub::link_admits;
use crate::hub::peer::{Expected, PeerError};
use crate::report;
use crate::token_store::{Credential, TokenStore, TokenStoreError};

pub const ASK_EVERY: Duration = Duration::from_mins(1);

pub struct Enrolled {
    pub config: Config,
    pub credential: Credential,
}

#[derive(Debug, thiserror::Error)]
pub enum MoveError {
    #[error("{0}")]
    Token(#[from] TokenStoreError),
    #[error("{0}")]
    Home(#[from] HomeError),
    #[error("{0}")]
    Peer(#[from] PeerError),
}

#[derive(Debug, PartialEq, Eq)]
enum Step {
    Stay,
    Ask(Origin),
    /// The token record already names the socket: a move stopped after its first write.
    Finish(Origin),
}

fn step(config: &Origin, credential: &Origin, socket: &Path) -> Step {
    if config.reach() != Reach::OwnLinkSocket {
        return Step::Stay;
    }
    // A path that is not text would be written as a different one.
    let Some(Ok(origin)) = socket
        .to_str()
        .map(|path| Origin::parse(&format!("unix:{path}")))
    else {
        return Step::Stay;
    };
    if *credential == origin {
        Step::Finish(origin)
    } else if credential == config {
        Step::Ask(origin)
    } else {
        Step::Stay
    }
}

impl Enrolled {
    #[must_use]
    pub fn is_old(&self) -> bool {
        self.config.hub.reach() == Reach::OwnLinkSocket
    }

    /// Writes the token record before the configuration, so a stop between the two is finished at the next start.
    pub async fn moved(
        self,
        home: &Home,
        store: &TokenStore,
        link: (&Path, &Expected),
    ) -> (Self, Option<MoveError>) {
        let (socket, expected) = link;
        let (origin, token_moves) = match step(&self.config.hub, &self.credential.hub, socket) {
            Step::Stay => return (self, None),
            Step::Finish(origin) => (origin, false),
            Step::Ask(origin) => {
                // The client of the run reports a socket that fails its checks; here nothing moves.
                if !matches!(link_admits(socket, expected).await, Ok(true)) {
                    return (self, None);
                }
                (origin, true)
            }
        };
        let credential = Credential {
            hub: origin.clone(),
            ..self.credential
        };
        if token_moves && let Err(error) = store.save(&credential) {
            let credential = Credential {
                hub: self.config.hub.clone(),
                ..credential
            };
            return (Self { credential, ..self }, Some(error.into()));
        }
        let config = Config {
            hub: origin,
            ..self.config.clone()
        };
        if let Err(error) = home.write_config(&config) {
            // The record in memory keeps the origin of the configuration, which the client uses.
            let credential = Credential {
                hub: self.config.hub.clone(),
                ..credential
            };
            return (Self { credential, ..self }, Some(error.into()));
        }
        (Self { config, credential }, None)
    }
}

pub struct Start {
    pub enrolled: Enrolled,
    pub watch: Watch,
    pub socket: PathBuf,
}

/// Until the move, an old enrolment sends through the socket of the move on macOS, as no value of
/// the environment decides where a token from the Keychain goes.
///
/// # Errors
/// [`MoveError::Peer`] when this daemon's signature cannot be read; [`MoveError::Home`] when the
/// account's home folder cannot be read (macOS).
pub async fn at_start(
    home: &Home,
    store: &TokenStore,
    enrolled: Enrolled,
) -> Result<Start, MoveError> {
    let own = home.link_socket();
    if !enrolled.is_old() {
        return Ok(Start {
            enrolled,
            watch: Watch::none(),
            socket: own,
        });
    }
    let expected = Expected::of_this_daemon()?;
    let socket = socket_of_move(home).map_err(HomeError::from)?;
    let client = if cfg!(target_os = "macos") {
        socket.clone()
    } else {
        own
    };
    let (enrolled, failed) = enrolled.moved(home, store, (&socket, &expected)).await;
    let asks = matches!(
        step(&enrolled.config.hub, &enrolled.credential.hub, &socket),
        Step::Ask(_)
    );
    let watch = match failed {
        Some(error) => {
            report::move_failed(&error);
            Watch::none()
        }
        None if !enrolled.is_old() => {
            report::moved();
            Watch::none()
        }
        None => {
            report::old_origin_waits(&client);
            if asks {
                Watch::of(socket, expected, ASK_EVERY)
            } else {
                Watch::none()
            }
        }
    };
    Ok(Start {
        enrolled,
        watch,
        socket: client,
    })
}

/// On macOS below the account's home folder, not the environment's folder, because the token is in the Keychain.
#[cfg_attr(
    not(target_os = "macos"),
    expect(
        clippy::unnecessary_wraps,
        reason = "the macOS form reads the user database, which can fail"
    )
)]
fn socket_of_move(home: &Home) -> std::io::Result<PathBuf> {
    #[cfg(target_os = "macos")]
    {
        let _ = home;
        crate::config::home::account_link_socket()
    }
    #[cfg(not(target_os = "macos"))]
    Ok(home.link_socket())
}

/// Waits for the program at the socket of the move to pass the check; the daemon then starts again and moves.
pub struct Watch(Option<(PathBuf, Expected, Duration)>);

impl Watch {
    #[must_use]
    pub const fn none() -> Self {
        Self(None)
    }

    #[must_use]
    pub const fn of(socket: PathBuf, expected: Expected, every: Duration) -> Self {
        Self(Some((socket, expected, every)))
    }

    pub async fn passed(&self) {
        let Some((socket, expected, every)) = &self.0 else {
            return std::future::pending().await;
        };
        loop {
            tokio::time::sleep(*every).await;
            if matches!(link_admits(socket, expected).await, Ok(true)) {
                return;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SOCKET: &str = "/Users/me/Library/Application Support/anima-daemon/link.sock";

    fn old() -> Origin {
        Origin::parse_kept("http://127.0.0.1:4100").expect("an old link origin")
    }

    fn moved_to() -> Origin {
        Origin::parse(&format!("unix:{SOCKET}")).expect("a link origin")
    }

    #[test]
    fn only_an_old_enrolment_with_the_same_origin_twice_asks_the_peer() {
        assert_eq!(
            step(&old(), &old(), Path::new(SOCKET)),
            Step::Ask(moved_to())
        );
        let https = Origin::parse("https://hub.example").expect("an origin");
        assert_eq!(step(&https, &https, Path::new(SOCKET)), Step::Stay);
        assert_eq!(
            step(&moved_to(), &moved_to(), Path::new(SOCKET)),
            Step::Stay
        );
        assert_eq!(step(&old(), &https, Path::new(SOCKET)), Step::Stay);
    }

    #[test]
    fn a_move_that_stopped_after_the_token_record_is_finished_without_a_new_check() {
        assert_eq!(
            step(&old(), &moved_to(), Path::new(SOCKET)),
            Step::Finish(moved_to())
        );
    }

    #[test]
    fn a_socket_path_that_is_no_link_origin_is_not_moved_to() {
        assert_eq!(
            step(&old(), &old(), Path::new("relative/link.sock")),
            Step::Stay
        );
        let not_text = Path::new(
            <std::ffi::OsStr as std::os::unix::ffi::OsStrExt>::from_bytes(
                b"/Users/m\xffe/link.sock",
            ),
        );
        assert_eq!(step(&old(), &old(), not_text), Step::Stay);
    }
}
