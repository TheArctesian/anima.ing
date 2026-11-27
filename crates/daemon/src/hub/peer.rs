//! The check of the peer at the socket of `anima-link`, made before the daemon writes a byte to
//! it (K8 Section 1, HMD-054). A same-user program that hands its socket or process id to the
//! signed link passes; the ruling that chose this check says so.

mod signature;

use std::io;

pub use signature::TeamId;
#[cfg(target_os = "macos")]
pub(crate) use signature::daemon_requirement;

#[cfg(target_os = "macos")]
pub(crate) fn release_daemon() -> Result<bool, PeerError> {
    use security_framework::os::macos::code_signing::{Flags, SecCode, SecRequirement};
    use std::sync::OnceLock;
    static SIGNED: OnceLock<bool> = OnceLock::new();
    if let Some(answer) = SIGNED.get() {
        return Ok(*answer);
    }
    let signed = match signature::own_program()? {
        Program::OfTheUser => false,
        Program::SignedBy(team) if Some(&team) != TeamId::parse("7GDLG48PV6").as_ref() => false,
        Program::SignedBy(_) => {
            let requirement: SecRequirement = daemon_requirement(std::process::id())
                .ok_or(PeerError::OwnSignature)?
                .parse()
                .map_err(|_| PeerError::OwnSignature)?;
            SecCode::for_self(Flags::NONE)
                .and_then(|code| code.check_validity(Flags::NONE, &requirement))
                .map_err(|_| PeerError::OwnSignature)?;
            true
        }
    };
    Ok(*SIGNED.get_or_init(|| signed))
}

#[derive(Debug, thiserror::Error)]
pub enum PeerError {
    #[error("the program at the socket of anima-link runs as a different user")]
    OtherUser,
    #[error("the program at the socket of anima-link is not the signed anima-link")]
    NotTheLink,
    #[error("the program at the socket of anima-link cannot be identified: {0}")]
    Io(#[from] io::Error),
    #[error(
        "the signature of this daemon cannot be read, or the file of the daemon is not the program that runs"
    )]
    OwnSignature,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Program {
    /// An unsigned daemon, and each daemon on Linux, where the token file is open to the user
    /// anyway (K8 Section 2).
    OfTheUser,
    SignedBy(TeamId),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Expected {
    pub user: u32,
    pub program: Program,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Peer {
    pub user: u32,
    pub process: Option<i32>,
}

impl Expected {
    /// # Errors
    /// [`PeerError::Io`] or [`PeerError::OwnSignature`] when the own user or signature cannot
    /// be read.
    pub fn of_this_daemon() -> Result<Self, PeerError> {
        let user = crate::config::private_file::running_user()?;
        #[cfg(target_os = "macos")]
        let program = signature::own_program()?;
        #[cfg(not(target_os = "macos"))]
        let program = Program::OfTheUser;
        Ok(Self { user, program })
    }

    /// # Errors
    /// [`PeerError::OtherUser`] or [`PeerError::NotTheLink`]; the caller then writes no request.
    pub async fn admit(&self, peer: Peer) -> Result<(), PeerError> {
        if peer.user != self.user {
            return Err(PeerError::OtherUser);
        }
        match &self.program {
            Program::OfTheUser => Ok(()),
            Program::SignedBy(team) => {
                let process = peer.process.ok_or(PeerError::NotTheLink)?;
                if signature::is_the_link(process, team).await? {
                    Ok(())
                } else {
                    Err(PeerError::NotTheLink)
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ME: u32 = 501;

    fn peer(user: u32) -> Peer {
        Peer {
            user,
            process: Some(1),
        }
    }

    #[tokio::test]
    async fn a_peer_of_a_different_user_is_refused_and_one_of_the_same_user_is_admitted() {
        let expected = Expected {
            user: ME,
            program: Program::OfTheUser,
        };
        assert!(matches!(
            expected.admit(peer(ME + 1)).await,
            Err(PeerError::OtherUser)
        ));
        assert!(expected.admit(peer(ME)).await.is_ok());
    }

    #[tokio::test]
    async fn a_signed_daemon_refuses_a_peer_whose_process_is_unknown() {
        let expected = Expected {
            user: ME,
            program: Program::SignedBy(TeamId::parse("ABCDE12345").expect("a team")),
        };
        let unknown = Peer {
            user: ME,
            process: None,
        };
        assert!(matches!(
            expected.admit(unknown).await,
            Err(PeerError::NotTheLink)
        ));
    }
}
