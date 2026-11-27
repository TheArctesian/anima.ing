//! Claim advertisement adds whole project/folder/program pairs to the existing K10 claim.

use daemon_core::wire::{Claim, Free, Held, Switch, WorkArchitecture, WorkOs};
use serde::Serialize;
use uuid::Uuid;

use super::policy::Target;

const LIMIT: usize = 8 * 1024;

pub struct Advertised<'a> {
    base: &'a Claim,
    threads: Switch,
    applications: Switch,
    types: Vec<&'static str>,
    kinds: Vec<&'static str>,
    thread_targets: Vec<&'a Target>,
}

// Flattening an unchanged existing Claim would duplicate the lists. Include its
// fields explicitly on the wire, retaining the existing meanings of each switch.
impl<'a> Advertised<'a> {
    #[must_use]
    pub fn new(
        base: &'a Claim,
        applications: Switch,
        threads: Switch,
        pairs: &'a [Target],
    ) -> Self {
        let mut types = base
            .types
            .iter()
            .map(|kind| kind.as_str())
            .collect::<Vec<_>>();
        let mut kinds = base
            .kinds
            .iter()
            .map(|kind| match kind {
                daemon_core::wire::WorkKind::Container => "container",
                daemon_core::wire::WorkKind::Native => "native",
                daemon_core::wire::WorkKind::Model => "model",
                daemon_core::wire::WorkKind::Unknown => "unknown",
            })
            .collect::<Vec<_>>();
        if threads == Switch::On {
            types.push("agent_thread");
            kinds.push("person");
        }
        let mut claim = Self {
            base,
            applications,
            threads,
            types,
            kinds,
            thread_targets: Vec::new(),
        };
        if threads == Switch::On {
            claim.pack(pairs);
        }
        claim
    }

    #[must_use]
    pub const fn device(&self) -> Uuid {
        self.base.device
    }

    #[must_use]
    pub const fn cadence(&self) -> std::time::Duration {
        if matches!(self.threads, Switch::On)
            || matches!(self.applications, Switch::On)
            || matches!(self.base.switch, Switch::On)
        {
            std::time::Duration::from_secs(10)
        } else {
            std::time::Duration::from_secs(3600)
        }
    }

    fn pack(&mut self, pairs: &'a [Target]) {
        for target in pairs {
            if self.thread_targets.contains(&target) {
                continue;
            }
            self.thread_targets.push(target);
            if self.encode().is_none() {
                self.thread_targets.pop();
            }
        }
    }

    pub(crate) fn encode(&self) -> Option<zeroize::Zeroizing<Vec<u8>>> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct ClaimWire<'a> {
            device: Uuid,
            switch: Switch,
            applications: Switch,
            threads: Switch,
            types: &'a [&'static str],
            kinds: &'a [&'static str],
            os: WorkOs,
            architecture: WorkArchitecture,
            free: Free,
            holds: &'a [Held],
            unclean: bool,
            thread_targets: &'a [&'a Target],
        }
        let wire = ClaimWire {
            device: self.base.device,
            switch: self.base.switch,
            applications: self.applications,
            threads: self.threads,
            types: &self.types,
            kinds: &self.kinds,
            os: self.base.os,
            architecture: self.base.architecture,
            free: self.base.free,
            holds: &self.base.holds,
            unclean: self.base.unclean,
            thread_targets: &self.thread_targets,
        };
        let bytes = zeroize::Zeroizing::new(serde_json::to_vec(&wire).ok()?);
        (bytes.len() <= LIMIT).then_some(bytes)
    }
}
