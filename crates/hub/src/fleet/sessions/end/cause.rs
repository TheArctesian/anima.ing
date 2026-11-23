//! Cause to close code, end reason and event severity for both tunnels (K4 Sections 5 and 8.6, R-032, D-9).

use std::time::Duration;

use crate::fleet::events::Severity;
use crate::fleet::sessions::{Actor, EndCause, EndReason, Protocol};

/// The client's idle limit and the longest one send to the client may take (K4 Sections 4 and 8.5).
pub(in crate::fleet::sessions) const CLIENT_IDLE: Duration = Duration::from_secs(15);
/// The wait for each peer while closing; it stays well inside `endSession`'s wait for the tunnel (D-10).
pub(in crate::fleet::sessions) const CLOSE_WAIT: Duration = Duration::from_secs(2);

#[derive(Debug, PartialEq, Eq)]
pub(in crate::fleet::sessions) struct Sentences {
    ssh: &'static str,
    rdp: &'static str,
    vnc: &'static str,
}

impl Sentences {
    pub(in crate::fleet::sessions) const fn of(&self, protocol: Protocol) -> &'static str {
        match protocol {
            Protocol::Ssh => self.ssh,
            Protocol::Rdp => self.rdp,
            Protocol::Vnc => self.vnc,
        }
    }
}

macro_rules! naming_the_protocol {
    ($before:literal, $after:literal) => {
        &Sentences {
            ssh: concat!($before, "SSH", $after),
            rdp: concat!($before, "RDP", $after),
            vnc: concat!($before, "VNC", $after),
        }
    };
    (same $text:literal) => {
        &Sentences {
            ssh: $text,
            rdp: $text,
            vnc: $text,
        }
    };
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::fleet::sessions) enum Words {
    Same(&'static str),
    Named(&'static Sentences),
}

impl Words {
    pub(in crate::fleet::sessions) const fn of(self, protocol: Protocol) -> &'static str {
        match self {
            Self::Same(text) => text,
            Self::Named(named) => named.of(protocol),
        }
    }
}

pub(in crate::fleet::sessions) const CLIENT_STOPPED_READING: &Sentences = naming_the_protocol!(
    "The client stopped reading what the hub sent, so the hub closed the ",
    " session."
);

pub(in crate::fleet::sessions) const CLIENT_LEFT: &Sentences =
    naming_the_protocol!("The client left before the ", " session opened.");

pub(in crate::fleet::sessions) const TASK_STOPPED: &Sentences = naming_the_protocol!(
    "The hub's tunnel stopped unexpectedly and closed the ",
    " session."
);

/// The first cause wins; the tunnel task records it once (D-9).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::fleet::sessions) enum Cause {
    OperatorClosed,
    RemoteClosed,
    GatewayError,
    AuthFailed,
    HostKeyMismatch,
    ServerCertificateMismatch,
    Timeout,
    HubShutdown,
    /// The client broke the tunnel's protocol (GW-011, K4 Section 8.4, R-039).
    InvalidInstruction,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::fleet::sessions) struct Ending {
    pub(in crate::fleet::sessions) close_code: u16,
    pub(in crate::fleet::sessions) reason: EndReason,
    pub(in crate::fleet::sessions) severity: Severity,
}

impl Cause {
    /// The K4 Section 5 row for this cause (R-034).
    pub(in crate::fleet::sessions) const fn ending(self) -> Ending {
        let (close_code, reason, severity) = match self {
            Self::OperatorClosed => (1000, EndReason::Operator, Severity::Info),
            Self::RemoteClosed => (1000, EndReason::RemoteClosed, Severity::Info),
            Self::GatewayError => (1011, EndReason::GatewayError, Severity::Warning),
            Self::AuthFailed => (1011, EndReason::AuthFailed, Severity::Warning),
            Self::HostKeyMismatch => (4001, EndReason::HostKeyMismatch, Severity::Error),
            Self::ServerCertificateMismatch => {
                (4003, EndReason::ServerCertificateMismatch, Severity::Error)
            }
            Self::Timeout => (4002, EndReason::Timeout, Severity::Info),
            Self::HubShutdown => (1001, EndReason::HubShutdown, Severity::Info),
            Self::InvalidInstruction => (1002, EndReason::ProtocolError, Severity::Warning),
        };
        Ending {
            close_code,
            reason,
            severity,
        }
    }

    /// An end the hub decided is recorded as Hub; `endSession` keeps its request's kind (LOG-014 rule 1).
    pub(in crate::fleet::sessions) const fn decided_by_hub(self) -> bool {
        match self {
            Self::OperatorClosed | Self::RemoteClosed => false,
            Self::AuthFailed
            | Self::GatewayError
            | Self::HostKeyMismatch
            | Self::ServerCertificateMismatch
            | Self::Timeout
            | Self::HubShutdown
            | Self::InvalidInstruction => true,
        }
    }

    /// Fixed text: never guacd's words, an address, a value or session content (LOG-007, RA-008).
    pub(in crate::fleet::sessions) const fn message(self) -> &'static Sentences {
        match self {
            Self::OperatorClosed => naming_the_protocol!("The operator closed the ", " session."),
            Self::RemoteClosed => {
                naming_the_protocol!("The device or the gateway closed the ", " session.")
            }
            Self::GatewayError => {
                naming_the_protocol!("The gateway reported an error and closed the ", " session.")
            }
            Self::AuthFailed => naming_the_protocol!("The device refused the ", " sign-in."),
            Self::HostKeyMismatch => {
                naming_the_protocol!(same
                    "The device presented an SSH host key that is not the accepted one, so the \
                     hub closed the session before signing in. Test the connection and check the \
                     new key before accepting it."
                )
            }
            Self::ServerCertificateMismatch => {
                naming_the_protocol!(same
                    "The device presented an RDP server certificate that is not the accepted \
                     one, so the hub closed the session before signing in. Test the connection \
                     and check the new certificate before accepting it."
                )
            }
            Self::Timeout => naming_the_protocol!(
                "The client sent nothing for too long, so the hub closed the ",
                " session."
            ),
            Self::HubShutdown => {
                naming_the_protocol!("The hub shut down and closed the ", " session.")
            }
            Self::InvalidInstruction => naming_the_protocol!(
                "The client sent an instruction the hub does not accept, so the hub closed the ",
                " session."
            ),
        }
    }
}

/// An oversize or non-UTF-8 message breaks the protocol (R-039); any other error is the client leaving.
pub(in crate::fleet::sessions) fn unreadable(error: axum::Error) -> Cause {
    match error
        .into_inner()
        .downcast::<tungstenite::Error>()
        .as_deref()
    {
        Ok(tungstenite::Error::Capacity(_) | tungstenite::Error::Utf8(_)) => {
            Cause::InvalidInstruction
        }
        _ => Cause::OperatorClosed,
    }
}

impl From<EndReason> for Cause {
    fn from(reason: EndReason) -> Self {
        match reason {
            EndReason::Operator => Self::OperatorClosed,
            EndReason::RemoteClosed => Self::RemoteClosed,
            EndReason::GatewayError => Self::GatewayError,
            EndReason::AuthFailed => Self::AuthFailed,
            EndReason::HostKeyMismatch => Self::HostKeyMismatch,
            EndReason::ServerCertificateMismatch => Self::ServerCertificateMismatch,
            EndReason::ProtocolError => Self::InvalidInstruction,
            EndReason::Timeout => Self::Timeout,
            EndReason::HubShutdown => Self::HubShutdown,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::fleet::sessions) struct End {
    pub(in crate::fleet::sessions) cause: Cause,
    pub(in crate::fleet::sessions) by: Option<Actor>,
    pub(in crate::fleet::sessions) message: Words,
}

impl End {
    pub(in crate::fleet::sessions) const fn explained(cause: Cause, message: &'static str) -> Self {
        Self {
            cause,
            by: None,
            message: Words::Same(message),
        }
    }

    pub(in crate::fleet::sessions) const fn worded(
        cause: Cause,
        message: &'static Sentences,
    ) -> Self {
        Self {
            cause,
            by: None,
            message: Words::Named(message),
        }
    }
}

impl From<Cause> for End {
    fn from(cause: Cause) -> Self {
        Self::worded(cause, cause.message())
    }
}

impl From<EndCause> for End {
    fn from(signal: EndCause) -> Self {
        let cause = Cause::from(signal.reason);
        Self {
            cause,
            by: signal.by,
            message: Words::Named(cause.message()),
        }
    }
}
