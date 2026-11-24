//! The tool inventory of a device (CAP-020 to CAP-028). Pure: the daemon runs the programs
//! (CAP-028); the hub cannot send a probe or a command line (CAP-021).

mod probes;
mod search;
#[cfg(test)]
mod tests;
mod tools;
mod version;

pub use probes::{PROBES, Probe, Program, TARGETS, build_targets};
pub use search::{TOOL_FOLDERS, candidates, may_run};
pub use tools::{Reason, Tool, ToolName, Verdict};
pub use version::{
    MAX_TEXT, is_build_target, is_tool_name, is_version, kernel_release, macos_version,
    os_release_version,
};

use serde::{Deserialize, Serialize};

pub const MAX_TOOLS: usize = 64;

pub const MAX_BUILD_TARGETS: usize = 128;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Inventory {
    pub remote_access: Option<RemoteAccess>,
    pub architecture: Architecture,
    pub os: Os,
    pub os_version: String,
    /// Each tool found once, in [`ToolName`] order. A tool not found is not listed.
    pub tools: Vec<Tool>,
    /// The installed Rust targets, unique, in byte order.
    pub build_targets: Vec<String>,
}

/// Private services installed and enabled by this computer's owner.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RemoteAccess {
    pub ssh_user: Option<String>,
    pub rdp_user: Option<String>,
}

impl RemoteAccess {
    #[must_use]
    pub fn accepted(&self) -> bool {
        [&self.ssh_user, &self.rdp_user]
            .into_iter()
            .flatten()
            .all(|user| {
                !user.is_empty()
                    && user.len() <= 64
                    && user
                        .bytes()
                        .all(|c| c.is_ascii_alphanumeric() || b"_-.".contains(&c))
            })
    }
}

/// The processor architecture, as Rust names it (CAP-026).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Architecture {
    Aarch64,
    X86_64,
}

impl Architecture {
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        match text {
            "aarch64" => Some(Self::Aarch64),
            "x86_64" => Some(Self::X86_64),
            _ => None,
        }
    }

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Aarch64 => "aarch64",
            Self::X86_64 => "x86_64",
        }
    }
}

/// The operating system, as Rust names it (CAP-026).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Os {
    Macos,
    Linux,
    Windows,
}

impl Os {
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        match text {
            "macos" => Some(Self::Macos),
            "linux" => Some(Self::Linux),
            "windows" => Some(Self::Windows),
            _ => None,
        }
    }

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Macos => "macos",
            Self::Linux => "linux",
            Self::Windows => "windows",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    Missing,
    TimedOut,
    /// The program was found but could not start, or exited with a failure.
    Exited,
    /// The program ran and exited with success; what it printed (at most the runner's cap).
    Printed(Vec<u8>),
}

impl Inventory {
    /// A probe whose program is missing leaves its tool out; one that failed or printed no version
    /// lists it as not usable, with the [`Reason`] (CAP-020).
    #[must_use]
    pub fn new(
        (architecture, os, os_version): (Architecture, Os, String),
        outcomes: &[(&Probe, Outcome)],
        targets: Option<&[u8]>,
    ) -> Self {
        let mut tools: Vec<Tool> = outcomes
            .iter()
            .filter_map(|(probe, outcome)| probe.tool(outcome))
            .collect();
        tools.sort_by_key(|tool| tool.name);
        tools.dedup_by_key(|tool| tool.name);
        Self {
            remote_access: None,
            architecture,
            os,
            os_version,
            tools,
            build_targets: targets.map_or_default(build_targets),
        }
    }
}
