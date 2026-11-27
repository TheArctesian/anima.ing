//! The distinct holder-only frames of remote Code. No person socket action carries these.

use agent_threads::drive::Mode;
use serde::{Deserialize, Serialize};

use super::{Action, Event};
use crate::remote_code::{
    broker::{Ask, Reply},
    launch::ProcessIdentity,
    policy::Target,
    wire::{ClaimedLaunch, DispatchRequest, NotCreated},
};

#[derive(Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub(in crate::threads) enum ToHolder {
    Open,
    Prove {
        nonce: uuid::Uuid,
    },
    Reserve {
        enrollment: Enrollment,
        claim: ClaimedLaunch,
    },
    Start(Box<Start>),
    Reply {
        reply: Reply,
    },
    Reset {
        request: DispatchRequest,
    },
    Recover {
        request: DispatchRequest,
    },
    Close {
        request: DispatchRequest,
        cause: NotCreated,
    },
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(in crate::threads) struct Enrollment {
    pub device: uuid::Uuid,
    pub origin: String,
}

impl Enrollment {
    pub(in crate::threads) fn of(config: &crate::config::Config) -> Self {
        Self {
            device: config.device,
            origin: config.hub.as_text(),
        }
    }

    pub(in crate::threads) fn matches(&self, config: &crate::config::Config) -> bool {
        self.device == config.device && self.origin == config.hub.as_text()
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(in crate::threads) struct Start {
    pub request: DispatchRequest,
    pub program: String,
    pub env: Vec<(String, String)>,
    pub mode: Mode,
    pub candidates: Vec<Target>,
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub(in crate::threads) enum ToRunner {
    Challenge {
        nonce: uuid::Uuid,
    },
    Ready,
    Reserved {
        request: DispatchRequest,
    },
    Ask {
        ask: Ask,
    },
    Created {
        request: DispatchRequest,
        process: ProcessIdentity,
    },
    Pending {
        request: DispatchRequest,
    },
    Refused,
}

impl ToHolder {
    pub(in crate::threads) fn action(&self) -> Option<Action> {
        serde_json::to_string(self)
            .ok()
            .map(|frame| Action::RemoteCode { frame })
    }
}

impl ToRunner {
    pub(in crate::threads) fn event(&self) -> Option<Event> {
        serde_json::to_string(self)
            .ok()
            .map(|frame| Event::RemoteCode { frame })
    }
}
