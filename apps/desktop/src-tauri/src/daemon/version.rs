//! The daemon version from `anima-daemon --version` and the replace rule of HMD-055; file times are not compared.

use std::{ffi::OsString, path::Path, sync::atomic::AtomicBool, time::Duration};

use super::run::{self, Run};

/// `--version`: the daemon prints `anima-daemon MAJOR.MINOR.PATCH` and touches nothing.
pub const VERSION_ARGS: [&str; 1] = ["--version"];

/// The most bytes read of the answer: the line is under 40.
const LONGEST: u64 = 128;

const NAME: &str = "anima-daemon ";

/// Major, minor and patch, compared as numbers; the patch counts the commits on `main` (R-201).
pub type Version = [u64; 3];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    Replace,
    Same,
    KeepNewer,
}

/// The version in a `--version` answer; nothing may follow the three numbers but a newline.
pub fn parse(answer: &[u8]) -> Option<Version> {
    let text = std::str::from_utf8(answer).ok()?;
    let text = text.strip_suffix('\n').unwrap_or(text);
    let mut parts = text.strip_prefix(NAME)?.split('.').map(|part| {
        part.bytes()
            .all(|byte| byte.is_ascii_digit())
            .then(|| part.parse::<u64>().ok())
            .flatten()
    });
    let found = [parts.next()??, parts.next()??, parts.next()??];
    parts.next().is_none().then_some(found)
}

/// An unreadable installed version counts as older: each daemon since the first that prints its version answers, so the copy is older or does not run (HMD-055).
pub fn decide(carried: Version, installed: Option<Version>) -> Decision {
    match installed {
        None => Decision::Replace,
        Some(installed) if carried > installed => Decision::Replace,
        Some(installed) if carried == installed => Decision::Same,
        Some(_) => Decision::KeepNewer,
    }
}

pub fn read(
    program: &Path,
    env: &[(&'static str, OsString)],
    deadline: Duration,
    stop: &AtomicBool,
) -> Option<Version> {
    let asked = Run {
        program,
        args: &VERSION_ARGS,
        input: None,
        env,
        deadline,
        stop,
    };
    run::output(&asked, LONGEST)
        .ok()
        .and_then(|answer| parse(&answer))
}
