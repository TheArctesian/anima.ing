//! `main` reads the process environment once; everything else takes it as a parameter.

use std::ffi::OsString;
use std::io::IsTerminal as _;
use std::path::PathBuf;

use zeroize::Zeroizing;

/// `Debug` is not derived: `token` is a secret.
pub struct Environment {
    /// `ANIMA_HUB` (CMD-013).
    pub hub: Option<String>,
    /// `ANIMA_TOKEN`, used before the stored token (CMD-013).
    pub token: Option<Zeroizing<String>>,
    /// `ANIMA_TOKEN_STORE=file`: keep the token in the 0600 file, not the OS key store.
    pub token_file: bool,
    /// Unix: `$XDG_CONFIG_HOME/anima` or `~/.config/anima`; Windows: `%LOCALAPPDATA%/Anima/cli`.
    pub config_dir: Option<PathBuf>,
    /// `NO_COLOR` is set and not empty (CMD-027).
    pub no_colour: bool,
    pub terminals: Terminals,
}

/// Which standard streams are terminals (CMD-023, CMD-027).
#[derive(Clone, Copy, Debug, Default)]
pub struct Terminals {
    pub stdin: bool,
    pub stdout: bool,
    /// Standard error: a link is for a person only when it is.
    pub stderr: bool,
}

impl Environment {
    #[must_use]
    pub fn from_process() -> Self {
        let value = variable;
        Self {
            hub: value("ANIMA_HUB").and_then(|value| value.into_string().ok()),
            token: value("ANIMA_TOKEN")
                .and_then(|value| value.into_string().ok())
                .map(Zeroizing::new),
            token_file: value("ANIMA_TOKEN_STORE").is_some_and(|value| value == "file"),
            config_dir: process_config_dir(),
            no_colour: value("NO_COLOR").is_some(),
            terminals: Terminals {
                stdin: std::io::stdin().is_terminal(),
                stdout: std::io::stdout().is_terminal(),
                stderr: std::io::stderr().is_terminal(),
            },
        }
    }
}

fn variable(name: &str) -> Option<OsString> {
    std::env::var_os(name).filter(|value| !value.is_empty())
}

#[must_use]
pub fn process_config_dir() -> Option<PathBuf> {
    if cfg!(windows) {
        return variable("LOCALAPPDATA")
            .map(PathBuf::from)
            .filter(|folder| folder.is_absolute())
            .map(|folder| folder.join("Anima/cli"));
    }
    config_dir(variable("XDG_CONFIG_HOME"), variable("HOME"))
}

/// The session files of this user's agent programs are below it (THR-007).
#[must_use]
pub fn process_home() -> Option<PathBuf> {
    let name = if cfg!(windows) { "USERPROFILE" } else { "HOME" };
    variable(name)
        .map(PathBuf::from)
        .filter(|home| home.is_absolute())
}

fn config_dir(xdg: Option<OsString>, home: Option<OsString>) -> Option<PathBuf> {
    let xdg = xdg.map(PathBuf::from).filter(|path| path.is_absolute());
    let base = xdg.or_else(|| home.map(|home| PathBuf::from(home).join(".config")))?;
    Some(base.join("anima"))
}

// XDG paths are the Unix configuration contract; Windows uses LOCALAPPDATA.
#[cfg(all(test, unix))]
mod tests {
    use super::*;

    #[test]
    fn cmd_015_the_config_folder_follows_xdg_then_home() {
        let dir = config_dir(Some("/x/cfg".into()), Some("/home/op".into()));
        assert_eq!(dir, Some(PathBuf::from("/x/cfg/anima")));
        let dir = config_dir(Some("relative".into()), Some("/home/op".into()));
        assert_eq!(dir, Some(PathBuf::from("/home/op/.config/anima")));
        assert_eq!(config_dir(None, None), None);
    }
}
