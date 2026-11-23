//! What the rules of a rollout read (K8 Section 8.1).

use crate::{fleet::builds::Version, ids::DeviceId};

/// The target of an installed daemon (OD-ROL-8).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Target {
    LinuxX8664,
    DarwinUniversal,
}

/// The fixed codes of K8 Section 8.1; `Failed` is each code that this hub does not know.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cause {
    RecordSignature,
    RecordMismatch,
    FileMismatch,
    LowerVersion,
    NotReplaceable,
    FetchFailed,
    NoSpace,
    StartFailed,
    NoCheck,
    Asked,
    Failed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    Installed,
    RolledBack(Cause),
    Refused(Cause),
}

/// `tries` counts the tries of the version since the operator last asked (ROL-032).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Try {
    pub version: Version,
    pub outcome: Outcome,
    pub tries: u8,
}

/// The `update` member of the newest accepted batch of a machine.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Said {
    /// `None` for a target that this hub does not know: no release is for that machine.
    pub target: Option<Target>,
    /// False when updates are set off at the machine (ROL-028). The hub cannot change it.
    pub switch_on: bool,
    /// The previous version that the machine keeps (ROL-017).
    pub previous: Option<Version>,
    /// True while the machine runs a job, a build or a job of a runner (ROL-028).
    pub busy: bool,
    pub last: Option<Try>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reach {
    Connected,
    /// Its metrics are stale (MET-018).
    NotConnected,
    /// Its daemon token ended, so the hub accepts no push from it (MET-013).
    TokenEnded,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Machine {
    pub device: DeviceId,
    /// The version that its daemon reported last (ROL-051).
    pub installed: Option<Version>,
    pub reach: Reach,
    pub with_hub: bool,
    /// True while the operator holds the machine (ROL-028).
    pub held: bool,
    /// `None` when the newest accepted batch had no `update` member (ROL-044).
    pub said: Option<Said>,
}

/// One verified release that the hub holds for one target (ROL-003, ROL-007).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Release {
    pub version: Version,
    pub target: Target,
    /// The lowest version that this release can replace (ROL-041).
    pub replaces_from: Version,
}
