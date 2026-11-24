//! The fixed version probes (CAP-021 to CAP-023, CAP-025); no probe starts a build, a model or a network request.

use std::collections::BTreeSet;

use super::{MAX_BUILD_TARGETS, Os, Outcome, Reason, Tool, ToolName, Verdict, version};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Program {
    pub name: &'static str,
    pub args: &'static [&'static str],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Probe {
    pub tool: ToolName,
    pub program: Program,
    pub only_on: Option<Os>,
    /// The program writes to its home folder even to print a version, so it gets an empty one of its own (CAP-025).
    pub own_home: bool,
    shape: Shape,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Shape {
    After(&'static str),
    LastWord(&'static str),
}

const fn probe(tool: ToolName, args: &'static [&'static str], shape: Shape) -> Probe {
    Probe {
        tool,
        program: Program {
            name: tool.as_str(),
            args,
        },
        only_on: None,
        own_home: false,
        shape,
    }
}

const VERSION: &[&str] = &["--version"];

/// Every probe, in [`ToolName`] order (CAP-023).
pub const PROBES: [Probe; 13] = [
    probe(ToolName::Git, VERSION, Shape::After("git version ")),
    probe(ToolName::Rustc, VERSION, Shape::After("rustc ")),
    probe(ToolName::Cargo, VERSION, Shape::After("cargo ")),
    probe(
        ToolName::CargoNextest,
        &["nextest", "--version"],
        Shape::After("cargo-nextest "),
    ),
    probe(ToolName::Sccache, VERSION, Shape::After("sccache ")),
    probe(ToolName::Bun, VERSION, Shape::After("")),
    probe(ToolName::Node, VERSION, Shape::After("v")),
    Probe {
        only_on: Some(Os::Macos),
        ..probe(ToolName::Xcodebuild, &["-version"], Shape::After("Xcode "))
    },
    probe(ToolName::Docker, VERSION, Shape::After("Docker version ")),
    probe(ToolName::Podman, VERSION, Shape::After("podman version ")),
    probe(ToolName::Nix, VERSION, Shape::LastWord("nix ")),
    Probe {
        own_home: true,
        ..probe(ToolName::Claude, VERSION, Shape::After(""))
    },
    Probe {
        own_home: true,
        ..probe(ToolName::Codex, VERSION, Shape::After("codex-cli "))
    },
];

/// The installed Rust targets, one to a line (CAP-023).
pub const TARGETS: Program = Program {
    name: "rustup",
    args: &["target", "list", "--installed"],
};

impl Probe {
    #[must_use]
    pub fn runs_on(&self, os: Os) -> bool {
        self.only_on.is_none_or(|only| only == os)
    }

    #[must_use]
    pub fn tool(&self, outcome: &Outcome) -> Option<Tool> {
        let verdict = match outcome {
            Outcome::Missing => return None,
            Outcome::TimedOut => Verdict::NotUsable(Reason::TimedOut),
            Outcome::Exited => Verdict::NotUsable(Reason::Exited),
            Outcome::Printed(output) => self
                .version(output)
                .map_or(Verdict::NotUsable(Reason::NoVersion), Verdict::Usable),
        };
        Some(Tool {
            name: self.tool,
            verdict,
        })
    }

    #[must_use]
    pub fn version(&self, output: &[u8]) -> Option<String> {
        let prefix = match self.shape {
            Shape::After(prefix) | Shape::LastWord(prefix) => prefix,
        };
        let line = version::text_lines(output).find(|line| line.starts_with(prefix))?;
        let rest = &line[prefix.len()..];
        let word = match self.shape {
            Shape::After(_) => rest.split_whitespace().next(),
            Shape::LastWord(_) => rest.split_whitespace().next_back(),
        }?;
        version::leading_version(word)
    }
}

/// Each line of `rustup target list --installed` that is a target, unique, in byte order.
#[must_use]
pub fn build_targets(output: &[u8]) -> Vec<String> {
    let found: BTreeSet<&str> = version::text_lines(output)
        .filter(|line| version::is_build_target(line))
        .collect();
    found
        .into_iter()
        .take(MAX_BUILD_TARGETS)
        .map(str::to_owned)
        .collect()
}
