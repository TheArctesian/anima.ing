//! Job fields come from the web view and become process arguments and path parts, so each parses into a safe type (BLD-027, BLD-041, BLD-043).

use super::error::BuilderError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Commit(String);

impl Commit {
    pub fn parse(text: &str) -> Result<Self, BuilderError> {
        let hex = text.len() == 40
            && text
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b));
        hex.then(|| Self(text.to_owned()))
            .ok_or(BuilderError::Commit)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// The same rule as the hub's (BLD-002), so the text is also a valid semver and a safe file name part.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Version(String);

impl Version {
    pub fn parse(text: &str) -> Result<Self, BuilderError> {
        let part = |p: &str| {
            !p.is_empty()
                && p.len() <= 9
                && p.bytes().all(|b| b.is_ascii_digit())
                && (p == "0" || !p.starts_with('0'))
        };
        let parts: Vec<&str> = text.split('.').collect();
        (parts.len() == 3 && parts.iter().all(|p| part(p)))
            .then(|| Self(text.to_owned()))
            .ok_or(BuilderError::Version)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// K1 `BuildTarget`: what a job builds (BLD-003).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    LinuxX86_64,
    DarwinUniversal,
    WindowsX86_64,
}

impl Target {
    pub fn parse(text: &str) -> Result<Self, BuilderError> {
        match text {
            "LINUX_X86_64" => Ok(Self::LinuxX86_64),
            "DARWIN_UNIVERSAL" => Ok(Self::DarwinUniversal),
            "WINDOWS_X86_64" => Ok(Self::WindowsX86_64),
            _ => Err(BuilderError::Target),
        }
    }

    pub fn buildable_on(self, os: &str, arch: &str) -> bool {
        match self {
            Self::LinuxX86_64 => os == "linux" && arch == "x86_64",
            Self::DarwinUniversal => os == "macos" && arch == "aarch64",
            Self::WindowsX86_64 => os == "windows" && arch == "x86_64",
        }
    }

    /// The folder of the target's files in the release bucket (BLD-060).
    pub const fn folder(self) -> &'static str {
        match self {
            Self::LinuxX86_64 => "linux-x86_64",
            Self::DarwinUniversal => "darwin-universal",
            Self::WindowsX86_64 => "windows-x86_64",
        }
    }

    pub fn here() -> Result<Self, BuilderError> {
        [
            Self::LinuxX86_64,
            Self::DarwinUniversal,
            Self::WindowsX86_64,
        ]
        .into_iter()
        .find(|target| target.buildable_on(std::env::consts::OS, std::env::consts::ARCH))
        .ok_or(BuilderError::Platform)
    }
}
