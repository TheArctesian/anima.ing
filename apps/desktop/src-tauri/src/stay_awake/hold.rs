//! Each hold is a child program that the OS ties to the app's process, so it ends when the app
//! ends, also after a failure (AWK-002, AWK-003, AWK-007).

use std::{
    ffi::OsStr,
    fs, io,
    os::unix::process::CommandExt,
    path::Path,
    process::{Child, Command, Stdio},
    thread,
    time::{Duration, Instant},
};

use nix::{
    sys::signal::{Signal, killpg},
    unistd::Pid,
};

use super::{
    platform::{Os, Platform},
    power,
};
use crate::os_program::{self, AfterLimit, Limit, Stdout};

/// A hold program that ends this soon after its start was refused by the OS.
const SETTLE: Duration = Duration::from_millis(400);

const END: Duration = Duration::from_secs(1);

const POLL: Duration = Duration::from_millis(10);

const PMSET_WRITE: Limit = Limit {
    patience: Duration::from_secs(5),
    after: AfterLimit::Kill,
};

/// `$0` is the followed process ID, `$1` is `sudo`, `$2` is `pmset`, `$3` is the marker.
/// The marker goes with the setting: a later start must not set back a setting that a different
/// program made.
const WATCHER: &str = r#"while /bin/kill -0 "$0" 2>/dev/null; do /bin/sleep 2; done; "$1" -n "$2" -a disablesleep 0 && /bin/rm -f "$3""#;

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum HoldError {
    #[error("the hold program could not start")]
    CouldNotStart,
    #[error("the OS refused the hold")]
    Refused,
    #[error("the set-up step is not done")]
    NotSetUp,
    #[error("the sleep setting could not be changed")]
    Setting,
}

/// Dropping it ends its process group.
#[derive(Debug)]
pub struct Hold(Child);

impl Hold {
    fn start(program: &str, args: &[&OsStr]) -> Result<Self, HoldError> {
        let child = Command::new(program)
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .process_group(0)
            .spawn()
            .map_err(|_| HoldError::CouldNotStart)?;
        let mut hold = Self(child);
        let started = Instant::now();
        while started.elapsed() < SETTLE {
            if !hold.alive() {
                return Err(HoldError::Refused);
            }
            thread::sleep(POLL);
        }
        Ok(hold)
    }

    pub fn alive(&mut self) -> bool {
        matches!(self.0.try_wait(), Ok(None))
    }
}

impl Hold {
    fn signal(&self, signal: Signal) {
        if let Ok(group) = i32::try_from(self.0.id()) {
            // The group is already gone when the program ended by itself.
            let _ = killpg(Pid::from_raw(group), signal);
        }
    }

    /// Only for the watcher, which ends by itself when the app's process ends.
    fn leave(self) {
        std::mem::forget(self);
    }
}

impl Drop for Hold {
    fn drop(&mut self) {
        self.signal(Signal::SIGTERM);
        let asked = Instant::now();
        while self.alive() && asked.elapsed() < END {
            thread::sleep(POLL);
        }
        // A program that ignores `SIGTERM` must not keep the hold, or the caller waiting.
        self.signal(Signal::SIGKILL);
        let _ = self.0.wait();
    }
}

/// The command waits for the followed process, so the inhibitor ends with the app.
fn inhibit_args(what: &str, followed: u32) -> Vec<String> {
    vec![
        format!("--what={what}"),
        "--who=Anima".into(),
        "--why=Stay awake".into(),
        "--mode=block".into(),
        "tail".into(),
        format!("--pid={followed}"),
        "-f".into(),
        "/dev/null".into(),
    ]
}

fn inhibit(platform: &Platform, what: &str) -> Result<Hold, HoldError> {
    let args = inhibit_args(what, platform.followed);
    let args: Vec<&OsStr> = args.iter().map(OsStr::new).collect();
    Hold::start(&platform.inhibit, &args)
}

/// The display can still sleep (AWK-002).
pub fn idle(platform: &Platform) -> Result<Hold, HoldError> {
    match platform.os {
        Os::Mac => {
            let followed = platform.followed.to_string();
            let args: [&OsStr; 3] = ["-i".as_ref(), "-w".as_ref(), followed.as_ref()];
            Hold::start(&platform.caffeinate, &args)
        }
        Os::Linux => inhibit(platform, "sleep"),
    }
}

/// What keeps the computer on with the lid closed (AWK-003).
#[derive(Debug)]
pub enum LidHold {
    Inhibitor(Hold),
    Setting {
        watcher: Hold,
    },
    /// A different program had already set `SleepDisabled`: Anima leaves it alone (AWK-007).
    Borrowed,
}

impl LidHold {
    /// Whether this hold remains active; a borrowed setting must be checked again.
    pub fn alive(&mut self) -> bool {
        match self {
            Self::Inhibitor(hold) | Self::Setting { watcher: hold } => hold.alive(),
            Self::Borrowed => false,
        }
    }
}

#[must_use]
pub fn sleep_disabled(report: &str) -> bool {
    report.lines().any(|line| {
        let mut words = line.split_whitespace();
        words.next() == Some("SleepDisabled") && words.next() == Some("1")
    })
}

fn set_sleep_disabled(platform: &Platform, disabled: bool) -> Result<(), HoldError> {
    let value = if disabled { "1" } else { "0" };
    let args: [&OsStr; 5] = [
        "-n".as_ref(),
        platform.pmset.as_ref(),
        "-a".as_ref(),
        "disablesleep".as_ref(),
        value.as_ref(),
    ];
    os_program::run(&platform.sudo, &args, PMSET_WRITE, Stdout::Discard)
        .map(drop)
        .map_err(|_| HoldError::Setting)
}

fn watcher(platform: &Platform, marker: &Path) -> Result<Hold, HoldError> {
    let followed = platform.followed.to_string();
    let args: [&OsStr; 6] = [
        "-c".as_ref(),
        WATCHER.as_ref(),
        followed.as_ref(),
        platform.sudo.as_ref(),
        platform.pmset.as_ref(),
        marker.as_ref(),
    ];
    Hold::start(&platform.shell, &args)
}

fn setting(platform: &Platform, marker: &Path) -> Result<LidHold, HoldError> {
    if !platform.rule.exists() {
        return Err(HoldError::NotSetUp);
    }
    let already =
        power::pmset_report(&platform.pmset, &["-g"]).is_some_and(|text| sleep_disabled(&text));
    if already && !marker.exists() {
        return Ok(LidHold::Borrowed);
    }
    // The marker is written first: a failure after the setting changed still leaves a record.
    let had_marker = marker.exists();
    fs::write(marker, b"").map_err(|_| HoldError::Setting)?;
    if let Err(error) = set_sleep_disabled(platform, true) {
        // A marker from before stays: the setting it records is still changed.
        if !had_marker {
            let _ = fs::remove_file(marker);
        }
        return Err(error);
    }
    match watcher(platform, marker) {
        Ok(watcher) => Ok(LidHold::Setting { watcher }),
        Err(error) => {
            let _ = restore(platform, marker);
            Err(error)
        }
    }
}

pub fn lid(platform: &Platform, marker: &Path) -> Result<LidHold, HoldError> {
    match platform.os {
        Os::Mac => setting(platform, marker),
        Os::Linux => inhibit(platform, "handle-lid-switch").map(LidHold::Inhibitor),
    }
}

/// The marker stays when the setting did not change, so the next start tries again.
pub fn restore(platform: &Platform, marker: &Path) -> Result<(), HoldError> {
    if !marker.exists() {
        return Ok(());
    }
    set_sleep_disabled(platform, false)?;
    match fs::remove_file(marker) {
        Err(error) if error.kind() != io::ErrorKind::NotFound => Err(HoldError::Setting),
        _ => Ok(()),
    }
}

/// Ends a lid hold (AWK-007).
pub fn release_lid(platform: &Platform, marker: &Path, hold: LidHold) -> Result<(), HoldError> {
    match hold {
        LidHold::Inhibitor(_) | LidHold::Borrowed => Ok(()),
        LidHold::Setting { watcher } => {
            let restored = restore(platform, marker);
            if restored.is_ok() {
                drop(watcher);
            } else {
                // The watcher stays: it sets the setting back when the app ends.
                watcher.leave();
            }
            restored
        }
    }
}
