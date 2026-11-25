//! Correlation on the trusted holder–daemon link. Wire replies carry evidence, not authority.
//!
//! Only the holder creates pending exchanges. Its connection generation is obtained from
//! the live holder link and must be checked again on receipt. Local person calls never enter here.

use daemon_core::wire::StopReason;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{
    launch::{DispatchExchange, DispatchGrant, LiveLease, PreparingReport},
    wire::DispatchRequest,
};
use crate::hub::remote_code::DispatchDecision;

#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    Prepare,
    Authorize,
}

#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct LinkExchange {
    generation: u64,
    id: Uuid,
    phase: Phase,
    request: DispatchRequest,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Ask(LinkExchange);

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Reply {
    exchange: LinkExchange,
    result: DispatchReply,
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "result", rename_all = "snake_case", deny_unknown_fields)]
pub(super) enum DispatchReply {
    Prepared { stop: Option<StopReason> },
    Dispatch { decision: DispatchDecision },
}

enum Timing {
    Prepare(PreparingReport),
    Authorize(DispatchExchange),
}

/// One local clock sample tied to a single exchange on a single connection.
pub struct Pending {
    exchange: LinkExchange,
    timing: Timing,
}

pub enum Matched {
    Live(LiveLease),
    Stopped(StopReason),
    Authorized(DispatchGrant),
    Final { authorization: Option<Uuid> },
    Conflict,
    TargetUnavailable,
}

impl Ask {
    #[must_use]
    pub const fn request(&self) -> &DispatchRequest {
        &self.0.request
    }

    #[must_use]
    pub const fn generation(&self) -> u64 {
        self.0.generation
    }

    #[must_use]
    pub const fn phase(&self) -> Phase {
        self.0.phase
    }

    #[must_use]
    pub const fn is_on(&self, generation: u64) -> bool {
        generation != 0 && self.0.generation == generation
    }

    pub(super) const fn reply(self, result: DispatchReply) -> Reply {
        Reply {
            exchange: self.0,
            result,
        }
    }
}

impl Pending {
    /// Sample before sending the ask over either the link or the HTTP transport.
    #[must_use]
    pub fn begin(generation: u64, phase: Phase, request: DispatchRequest) -> Option<(Self, Ask)> {
        if generation == 0 {
            return None;
        }
        let timing = match phase {
            Phase::Prepare => Timing::Prepare(PreparingReport::begin()?),
            Phase::Authorize => Timing::Authorize(DispatchExchange::begin(request)?),
        };
        let exchange = LinkExchange {
            generation,
            id: Uuid::now_v7(),
            phase,
            request,
        };
        Some((Self { exchange, timing }, Ask(exchange)))
    }

    #[must_use]
    pub(crate) fn matches(&self, generation: u64, reply: &Reply) -> bool {
        generation == self.exchange.generation && reply.exchange == self.exchange
    }

    /// Use only for a reply read on the current trusted holder link. A replaced connection or
    /// a mismatched phase, exchange ID or tuple never authorizes a process start.
    #[must_use]
    pub fn receive(self, generation: u64, reply: Reply) -> Option<Matched> {
        if !self.matches(generation, &reply) {
            return None;
        }
        match (self.timing, reply.result) {
            (Timing::Prepare(_), DispatchReply::Prepared { stop: Some(reason) }) => {
                Some(Matched::Stopped(reason))
            }
            (Timing::Prepare(sample), DispatchReply::Prepared { stop: None }) => {
                Some(Matched::Live(sample.accepted(self.exchange.request)))
            }
            (
                Timing::Authorize(exchange),
                DispatchReply::Dispatch {
                    decision: DispatchDecision::Authorized { grant },
                },
            ) => exchange.accepted(grant).map(Matched::Authorized),
            (
                Timing::Authorize(_),
                DispatchReply::Dispatch {
                    decision: DispatchDecision::Final { authorization },
                },
            ) => Some(Matched::Final { authorization }),
            (
                Timing::Authorize(_),
                DispatchReply::Dispatch {
                    decision: DispatchDecision::Conflict,
                },
            ) => Some(Matched::Conflict),
            (
                Timing::Authorize(_),
                DispatchReply::Dispatch {
                    decision: DispatchDecision::TargetUnavailable,
                },
            ) => Some(Matched::TargetUnavailable),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests;
