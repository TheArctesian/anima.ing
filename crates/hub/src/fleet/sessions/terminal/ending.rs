//! How a terminal session ends, in the terminal's words (K4 Section 8.6): the shared end
//! table (`end::cause`) with event messages that name the device and the shell, not a
//! gateway. Pure. Every message is a fixed sentence with no device text, address, value or
//! session content (LOG-007, RA-008).

use crate::fleet::sessions::{
    Protocol, credential_moved,
    end::{Cause, End},
};

pub(super) const NOT_TERMINAL: &str = "The client did not open the tunnel as a terminal \
                                       WebSocket, so the hub closed the SSH session.";
/// No credential is stored for the device (D-3).
pub(super) const NO_CREDENTIAL: &str = "No SSH credential is stored for the device, so the hub \
                                        could not sign in. Store one in the device's SSH \
                                        settings.";
/// The stored credential was entered for another host than the device's host now (SEC-011).
pub(super) const CREDENTIAL_MOVED: &str = credential_moved(Protocol::Ssh);
pub(super) const UNREADABLE_KEY: &str = "The stored SSH private key or its passphrase cannot be \
                                         read, so the hub could not sign in.";
pub(super) const NO_SIZE: &str = "The client did not send its terminal size in time, so the \
                                  hub closed the SSH session.";
pub(super) const PTY_REFUSED: &str =
    "The device refused a terminal for the session, so the hub closed the SSH session.";
pub(super) const SHELL_REFUSED: &str =
    "The device refused to start a shell, so the hub closed the SSH session.";
pub(super) const NO_ANSWER: &str = "The device did not answer the hub's request for a terminal \
                                    or shell in time, so the hub closed the SSH session.";
/// Client input waited on the device past the request wait.
pub(super) const DEVICE_STOPPED_READING: &str = "The device stopped reading what the operator \
                                                 typed, so the hub closed the SSH session.";

const REMOTE_CLOSED: &str = "The shell exited or the device closed the SSH session.";
const CONNECTION_LOST: &str =
    "The SSH connection to the device failed, so the hub closed the session.";
/// A client message outside K4 Section 8.4 or over a limit of Section 8.5.
const BAD_MESSAGE: &str =
    "The client sent a message the hub does not accept, so the hub closed the SSH session.";

pub(super) const fn ended(cause: Cause) -> End {
    match cause {
        Cause::RemoteClosed => End::explained(cause, REMOTE_CLOSED),
        Cause::GatewayError => End::explained(cause, CONNECTION_LOST),
        Cause::InvalidInstruction => End::explained(cause, BAD_MESSAGE),
        Cause::OperatorClosed
        | Cause::AuthFailed
        | Cause::HostKeyMismatch
        | Cause::ServerCertificateMismatch
        | Cause::Timeout
        | Cause::HubShutdown => End::worded(cause, cause.message()),
    }
}
