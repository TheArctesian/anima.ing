//! The tools the hub knows by name (CAP-023, CAP-024); the hub ignores a name it does not know (K8 Section 4).

use serde::{Deserialize, Serialize};

/// A tool of the inventory. The order is the order of the list the daemon sends.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ToolName {
    Git,
    Rustc,
    Cargo,
    CargoNextest,
    Sccache,
    Bun,
    Node,
    Xcodebuild,
    Docker,
    Podman,
    Nix,
    Claude,
    Codex,
}

impl ToolName {
    pub const ALL: [Self; 13] = [
        Self::Git,
        Self::Rustc,
        Self::Cargo,
        Self::CargoNextest,
        Self::Sccache,
        Self::Bun,
        Self::Node,
        Self::Xcodebuild,
        Self::Docker,
        Self::Podman,
        Self::Nix,
        Self::Claude,
        Self::Codex,
    ];

    /// The name a batch and the API spell, which is also the program's name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Git => "git",
            Self::Rustc => "rustc",
            Self::Cargo => "cargo",
            Self::CargoNextest => "cargo-nextest",
            Self::Sccache => "sccache",
            Self::Bun => "bun",
            Self::Node => "node",
            Self::Xcodebuild => "xcodebuild",
            Self::Docker => "docker",
            Self::Podman => "podman",
            Self::Nix => "nix",
            Self::Claude => "claude",
            Self::Codex => "codex",
        }
    }

    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|tool| tool.as_str() == text)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Tool {
    pub name: ToolName,
    pub verdict: Verdict,
}

/// A found tool's version, or why it is not usable (CAP-002).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Verdict {
    Usable(String),
    NotUsable(Reason),
}

/// A fixed code, never text from the device; K8 spells it as the `state` of a tool.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Reason {
    TimedOut,
    /// The program could not start, or exited with a failure.
    Exited,
    NoVersion,
    #[serde(other)]
    Failed,
}

impl Reason {
    /// The code K8 and the database spell.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::TimedOut => "timed_out",
            Self::Exited => "exited",
            Self::NoVersion => "no_version",
            Self::Failed => "failed",
        }
    }

    #[must_use]
    pub fn parse(text: &str) -> Self {
        [Self::TimedOut, Self::Exited, Self::NoVersion]
            .into_iter()
            .find(|reason| reason.as_str() == text)
            .unwrap_or(Self::Failed)
    }
}

impl Tool {
    #[must_use]
    pub fn version(&self) -> Option<&str> {
        match &self.verdict {
            Verdict::Usable(version) => Some(version),
            Verdict::NotUsable(_) => None,
        }
    }

    #[must_use]
    pub const fn reason(&self) -> Option<Reason> {
        match self.verdict {
            Verdict::Usable(_) => None,
            Verdict::NotUsable(reason) => Some(reason),
        }
    }

    #[must_use]
    pub const fn is_usable(&self) -> bool {
        matches!(self.verdict, Verdict::Usable(_))
    }
}
