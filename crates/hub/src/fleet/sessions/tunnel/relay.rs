//! The relay's view of one live tunnel after guacd's `ready` (K4 Section 4, D-7, D-8). Pure.
//!
//! The hub keeps only the credential names guacd asked for, never a value the operator types
//! (GW-015). A client `blob` or `end` is forwarded only on a stream the client opened with an
//! allowed instruction and has not ended. A guacd stream the policy withholds has its `blob` and
//! `end` dropped until guacd ends it or reopens its index. Other guacd streams are not tracked:
//! guacd ignores an `ack` for a stream it has not open (guacamole-server 1.5.5 and 1.6.0
//! `src/libguac/user-handlers.c`, `__guac_handle_ack`).

use std::collections::BTreeSet;

use guac_protocol::Instruction;

use super::ending::Cause;
use super::policy::Policy;

#[derive(Debug, PartialEq, Eq)]
pub(super) enum ClientStep {
    Forward(Instruction),
    /// Send this back to the client, not to guacd (the internal ping, GW-011).
    Reply(Instruction),
    Drop,
    /// Send `forward` to guacd if there is one, then end the session for `cause`.
    End {
        forward: Option<Instruction>,
        cause: Cause,
    },
}

#[derive(Debug, PartialEq, Eq)]
pub(super) enum GatewayStep {
    Forward(Instruction),
    Drop,
    /// Send this `ack` back to guacd, which closes a stream that waits for the client's `ack`.
    Refuse(Instruction),
    /// Send `send` to the client, then end the session for `cause`.
    End {
        send: Instruction,
        cause: Cause,
    },
}

/// Bounded: at most the policy's prompt names and [`MAX_STREAMS`] indices in each stream set.
#[derive(Debug)]
pub(super) struct Relay {
    pub(super) policy: Policy,
    pub(super) prompted: BTreeSet<&'static str>,
    pub(super) client_streams: BTreeSet<u16>,
    pub(super) dropped_client_streams: BTreeSet<u16>,
    pub(super) withheld_streams: BTreeSet<u16>,
}

impl Relay {
    pub(super) const fn new(policy: Policy) -> Self {
        Self {
            policy,
            prompted: BTreeSet::new(),
            client_streams: BTreeSet::new(),
            dropped_client_streams: BTreeSet::new(),
            withheld_streams: BTreeSet::new(),
        }
    }
}

/// `GUAC_USER_MAX_STREAMS` of guacamole-server 1.6.0; 1.5.5 has 64 and refuses client streams
/// 64..512 itself (`src/libguac/guacamole/user-constants.h:38`).
pub(super) const MAX_STREAMS: u16 = 512;
