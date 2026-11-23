//! The answer to a server-trust challenge (D-4, WEB-008). Pure: the platform hook only carries it.

use super::{
    fingerprint::Fingerprint,
    pin::{Challenge, Host, Pin, PinnedOrigin, Verdict, verdict},
};

/// A challenge without a port is TLS, which defaults to 443.
const TLS_PORT: u16 = 443;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Presented<'a> {
    pub host: &'a str,
    /// 0 means none was set.
    pub port: isize,
    /// `None` if the server presented no chain.
    pub leaf: Option<Fingerprint>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Answer {
    Accept,
    /// Stop the connection; nothing from it loads.
    Cancel,
    Default,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ruling {
    pub answer: Answer,
    /// `Untrusted` for the tab's own origin, or `Mismatch`.
    pub shown: Option<Verdict>,
}

impl Ruling {
    const fn quiet(answer: Answer) -> Self {
        Self {
            answer,
            shown: None,
        }
    }
}

/// Fails closed: no leaf, no host or a port out of range cancels.
/// `system_trust` is asked only with no pin: a pin decides alone (D-4), and evaluating trust can block.
/// An untrusted certificate is shown only for the tab's own origin; on any other host it is cancelled.
pub fn rule(
    pin: Option<&Pin>,
    origin: Option<&PinnedOrigin>,
    presented: &Presented<'_>,
    system_trust: impl FnOnce() -> bool,
) -> Ruling {
    let host = presented.host;
    let (Some(leaf), Some(port), Some(_)) = (
        presented.leaf,
        port_of(presented.port),
        Host::of_challenge(host),
    ) else {
        return Ruling::quiet(Answer::Cancel);
    };
    let challenge = Challenge {
        host,
        port,
        presented: leaf,
        system_trusted: pin.is_none() && system_trust(),
    };
    match verdict(pin, &challenge) {
        Verdict::Accept => Ruling::quiet(Answer::Accept),
        Verdict::Default => Ruling::quiet(Answer::Default),
        Verdict::Untrusted(_) if !origin.is_some_and(|origin| origin.holds(host, port)) => {
            Ruling::quiet(Answer::Cancel)
        }
        shown @ (Verdict::Untrusted(_) | Verdict::Mismatch { .. }) => Ruling {
            answer: Answer::Cancel,
            shown: Some(shown),
        },
    }
}

fn port_of(port: isize) -> Option<u16> {
    match port {
        0 => Some(TLS_PORT),
        port => u16::try_from(port).ok(),
    }
}
