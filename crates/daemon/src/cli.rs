//! The command line (MET-012, MET-019, HMD-055, HMD-056, K-E5 Section 2.1).

use std::io::{BufRead, Read as _};
use std::path::PathBuf;

use daemon_core::wire::{EnrolmentCode, is_key_name};
use zeroize::Zeroizing;

use crate::config::origin::{Origin, OriginError};
use crate::units::plan::Target;

pub const USAGE: &str = "usage: anima-daemon enrol <code | -> [--hub <origin>]
       anima-daemon run [--hosted-hub <data folder>]
       anima-daemon install [--system]
       anima-daemon uninstall
       anima-daemon stop
       anima-daemon work <on | off>
       anima-daemon key read <name>
       anima-daemon key forget
       anima-daemon threads
       anima-daemon holder <folder> [<bytes kept for each thread>]
       anima-daemon --version";

/// The desktop app reads this line to compare versions (R-122, HMD-055), so its form does not
/// change.
#[must_use]
pub fn version_line() -> String {
    format!("anima-daemon {}", env!("CARGO_PKG_VERSION"))
}

pub const ARGV_WARNING: &str = "warning: a code on the command line shows in process lists and shell history; next time run `anima-daemon enrol -` and paste the code at the hidden prompt";

pub enum Command {
    Enrol {
        code: CodeSource,
        hub: Origin,
    },
    /// `hosted_hub` is the data folder of the hosted hub whose container runs this daemon
    /// (HMD-071).
    Run {
        hosted_hub: Option<PathBuf>,
    },
    Install {
        target: Target,
    },
    Uninstall,
    Stop,
    /// The switch at the machine (USE-004).
    Work {
        on: bool,
    },
    KeyRead {
        name: String,
    },
    KeyForget,
    Threads,
    /// Started by the daemon, not the operator (THR-113).
    Holder {
        folder: std::path::PathBuf,
        limit: usize,
    },
    /// Prints [`version_line`]: no home, no token and no network.
    Version,
}

pub enum CodeSource {
    Given(EnrolmentCode),
    Stdin,
}

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum CliError {
    #[error("{USAGE}")]
    Usage,
    #[error("that is not an enrolment code (it starts with anima_enrol_)")]
    NotACode,
    #[error("--hub: {0}")]
    Hub(#[from] OriginError),
    #[error("that is not a key name (a-z, 0-9, _, . and -, starting with a letter)")]
    NotAKeyName,
}

/// # Errors
/// [`CliError`] when the arguments do not read; no error holds the code.
pub fn parse(args: impl IntoIterator<Item = String>) -> Result<Command, CliError> {
    let mut args = args.into_iter();
    match args.next().as_deref() {
        Some("run") => run(args),
        Some("uninstall") if args.next().is_none() => Ok(Command::Uninstall),
        Some("stop") if args.next().is_none() => Ok(Command::Stop),
        Some("work") => work(args),
        Some("enrol") => enrol(args),
        Some("install") => install(args),
        Some("key") => key(args),
        Some("holder") => holder(args),
        Some("threads") if args.next().is_none() => Ok(Command::Threads),
        Some("--version") if args.next().is_none() => Ok(Command::Version),
        _ => Err(CliError::Usage),
    }
}

/// A limit of 0 means the default.
fn holder(mut args: impl Iterator<Item = String>) -> Result<Command, CliError> {
    let folder = args
        .next()
        .filter(|folder| std::path::Path::new(folder).is_absolute());
    let limit = match args.next() {
        None => Some(0),
        Some(bytes) => bytes.parse().ok(),
    };
    match (folder, limit, args.next()) {
        (Some(folder), Some(limit), None) => Ok(Command::Holder {
            folder: folder.into(),
            limit,
        }),
        _ => Err(CliError::Usage),
    }
}

fn run(mut args: impl Iterator<Item = String>) -> Result<Command, CliError> {
    let hosted_hub = match (args.next().as_deref(), args.next()) {
        (None, _) => None,
        (Some("--hosted-hub"), Some(folder)) if folder.starts_with('/') => {
            Some(PathBuf::from(folder))
        }
        _ => return Err(CliError::Usage),
    };
    if args.next().is_some() {
        return Err(CliError::Usage);
    }
    Ok(Command::Run { hosted_hub })
}

fn work(mut args: impl Iterator<Item = String>) -> Result<Command, CliError> {
    let on = match args.next().as_deref() {
        Some("on") => true,
        Some("off") => false,
        _ => return Err(CliError::Usage),
    };
    if args.next().is_some() {
        return Err(CliError::Usage);
    }
    Ok(Command::Work { on })
}

fn key(mut args: impl Iterator<Item = String>) -> Result<Command, CliError> {
    let command = match (args.next().as_deref(), args.next()) {
        (Some("read"), Some(name)) if is_key_name(&name) => Command::KeyRead { name },
        (Some("read"), Some(_)) => return Err(CliError::NotAKeyName),
        (Some("forget"), None) => return Ok(Command::KeyForget),
        _ => return Err(CliError::Usage),
    };
    if args.next().is_some() {
        return Err(CliError::Usage);
    }
    Ok(command)
}

fn install(mut args: impl Iterator<Item = String>) -> Result<Command, CliError> {
    let target = match args.next().as_deref() {
        None => Target::User,
        Some("--system") => Target::System,
        Some(_) => return Err(CliError::Usage),
    };
    if args.next().is_some() {
        return Err(CliError::Usage);
    }
    Ok(Command::Install { target })
}

fn enrol(args: impl Iterator<Item = String>) -> Result<Command, CliError> {
    let mut code = None;
    let mut hub = None;
    let mut args = args;
    while let Some(arg) = args.next() {
        if arg == "--hub" {
            hub = Some(args.next().ok_or(CliError::Usage)?);
        } else if let Some(text) = arg.strip_prefix("--hub=") {
            hub = Some(text.to_owned());
        } else if code.is_none() && (arg == "-" || !arg.starts_with("--")) {
            code = Some(arg);
        } else {
            return Err(CliError::Usage);
        }
    }
    let code = match code.ok_or(CliError::Usage)?.as_str() {
        "-" => CodeSource::Stdin,
        text => CodeSource::Given(EnrolmentCode::parse(text).ok_or(CliError::NotACode)?),
    };
    let hub = match hub {
        Some(text) => Origin::parse(&text)?,
        None => Origin::default_hub(),
    };
    Ok(Command::Enrol { code, hub })
}

/// The most bytes read for a code from standard input: a code is under 40.
const CODE_INPUT_LIMIT: usize = 256;

/// # Errors
/// [`CliError::NotACode`] when the first line is not a code; the error holds none of the input.
///
/// The `StdinLock`'s own buffer is not wiped (LOG-007); the process is short-lived.
pub fn read_code(mut input: impl BufRead) -> Result<EnrolmentCode, CliError> {
    // The buffer has room for every byte that can come, so it never moves and is wiped whole.
    let mut line = Zeroizing::new(String::with_capacity(CODE_INPUT_LIMIT + 1));
    input
        .by_ref()
        .take(CODE_INPUT_LIMIT as u64)
        .read_line(&mut line)
        .map_err(|_| CliError::NotACode)?;
    code_from_line(&line)
}

/// # Errors
/// [`CliError::NotACode`] when the rest is not a code.
pub fn code_from_line(line: &str) -> Result<EnrolmentCode, CliError> {
    let text = line.strip_suffix('\n').unwrap_or(line);
    let text = text.strip_suffix('\r').unwrap_or(text);
    EnrolmentCode::parse(text).ok_or(CliError::NotACode)
}

#[cfg(test)]
mod tests;
