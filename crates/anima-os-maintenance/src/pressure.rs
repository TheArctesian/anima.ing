//! Reclaim from the agent slice when the hosting slice starts stalling.
use std::{
    fs::{self, OpenOptions},
    io::{self, Write},
    os::unix::fs::OpenOptionsExt,
    path::Path,
    time::{Duration, Instant},
};

use crate::{Error, read_small_receipt};

const HOSTING_PSI: &str = "/sys/fs/cgroup/anima.slice/anima-hosting.slice/memory.pressure";
const AGENT_HIGH: &str = "/sys/fs/cgroup/anima.slice/anima-agents.slice/memory.high";
const THROTTLED: &str = "/run/anima-hosting-pressure/active";
const LOCK: &str = "/run/anima-hosting-pressure/lock";
const PENDING: &str = "/run/anima-hosting-pressure/pending";

const CLEAR_SAMPLE: &str = "/run/anima-hosting-pressure/clear";
const CLEAR_PENDING: &str = "/run/anima-hosting-pressure/clear.pending";
const SAMPLE_MAX_AGE: u64 = 15;

const START_PERCENT: f64 = 2.0;
const RECOVERY_PERCENT: f64 = 0.5;
const RECOVERY_TIME: Duration = Duration::from_secs(30);

pub(super) enum Observation {
    Holding,
    Stale,
    Clear,
}

/// Check that the hosting pressure controller has a fresh, clear sample.
///
/// # Errors
/// Hosting is under pressure or the current observation is unavailable.
pub fn require_clear() -> Result<(), Error> {
    match observe()? {
        Observation::Holding => Err(Error::HostingPressure),
        Observation::Stale => Err(Error::PressureUnavailable),
        Observation::Clear => Ok(()),
    }
}

pub(super) fn observe() -> Result<Observation, Error> {
    let observation = read_small_receipt::<u64>(CLEAR_SAMPLE);
    if Path::new(THROTTLED).try_exists()? {
        return Ok(Observation::Holding);
    }
    if sample_is_fresh(observation?, boot_seconds()?) {
        Ok(Observation::Clear)
    } else {
        Ok(Observation::Stale)
    }
}

fn sample_is_fresh(observation: Option<u64>, now: u64) -> bool {
    observation
        .and_then(|sample| now.checked_sub(sample))
        .is_some_and(|age| age < SAMPLE_MAX_AGE)
}

fn boot_seconds() -> Result<u64, Error> {
    let time =
        nix::time::clock_gettime(nix::time::ClockId::CLOCK_BOOTTIME).map_err(io::Error::from)?;
    u64::try_from(time.tv_sec())
        .map_err(|_| Error::Refused("The hosting pressure clock is unavailable."))
}

fn publish_clear(sampled_at: u64) -> Result<(), Error> {
    let mut file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .custom_flags(nix::libc::O_NOFOLLOW)
        .open(CLEAR_PENDING)?;
    write!(file, "{sampled_at}")?;
    fs::rename(CLEAR_PENDING, CLEAR_SAMPLE)?;
    Ok(())
}

fn lock_controller() -> Result<fs::File, Error> {
    let lock = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .custom_flags(nix::libc::O_NOFOLLOW)
        .open(LOCK)?;
    lock.try_lock()
        .map_err(|_| Error::Refused("Another process owns hosting pressure control."))?;
    match fs::remove_file(CLEAR_SAMPLE) {
        Ok(()) => (),
        Err(error) if error.kind() == io::ErrorKind::NotFound => (),
        Err(error) => return Err(error.into()),
    }
    Ok(lock)
}

/// Sample hosted work and adjust only the agent slice's soft memory limit.
///
/// # Errors
/// Unavailable cgroup observations or failed writes.
pub async fn watch() -> Result<(), Error> {
    let _lock = lock_controller()?;
    let page_size = nix::unistd::sysconf(nix::unistd::SysconfVar::PAGE_SIZE)
        .map_err(io::Error::from)?
        .and_then(|size| u64::try_from(size).ok())
        .filter(|size| *size > 0)
        .ok_or(Error::Refused("The host page size is unavailable."))?;
    let mut throttle = match fs::read_to_string(THROTTLED) {
        Ok(record) => Some(Throttle::parse(&record, page_size)?),
        Err(error) if error.kind() == io::ErrorKind::NotFound => None,
        Err(error) => return Err(error.into()),
    };
    let mut quiet_since = None;
    let mut last_sample = 0;
    let mut ticks = tokio::time::interval(Duration::from_secs(5));
    ticks.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    loop {
        ticks.tick().await;
        let sampled_at = boot_seconds()?;
        if sampled_at.saturating_sub(last_sample) >= SAMPLE_MAX_AGE {
            quiet_since = None;
        }
        last_sample = sampled_at;
        let pressure = some_avg10(&fs::read_to_string(HOSTING_PSI)?)?;
        if pressure >= START_PERCENT && throttle.is_none() {
            let original = fs::read_to_string(AGENT_HIGH)?;
            let limit = Throttle::parse(&original, page_size)?;
            fs::write(PENDING, limit.normal.to_string())?;
            fs::rename(PENDING, THROTTLED)?;
            throttle = Some(limit);
            eprintln!("Hosting memory pressure paused new agent and build reservations.");
        }
        if let Some(limit) = &throttle {
            if recovered(&mut quiet_since, Instant::now(), pressure) {
                limit.restore()?;
                fs::remove_file(THROTTLED)?;
                throttle = None;
                quiet_since = None;
                eprintln!("Hosting memory pressure cleared.");
            } else {
                limit.apply()?;
            }
        }
        if throttle.is_none() {
            publish_clear(sampled_at)?;
        }
    }
}

struct Throttle {
    normal: u64,
    reduced: u64,
}

impl Throttle {
    fn parse(value: &str, page_size: u64) -> Result<Self, Error> {
        let normal = value
            .trim()
            .parse::<u64>()
            .ok()
            .filter(|bytes| *bytes > 0 && bytes.is_multiple_of(page_size))
            .ok_or(Error::Refused(
                "The managed agent memory limit is unavailable.",
            ))?;
        let reduced = (normal / page_size / 2).max(1) * page_size;
        Ok(Self { normal, reduced })
    }

    fn apply(&self) -> Result<(), Error> {
        if agent_high()? > self.reduced {
            set_agent_high(self.reduced)?;
        }
        Ok(())
    }

    fn restore(&self) -> Result<(), Error> {
        if self.normal > self.reduced && agent_high()? == self.reduced {
            set_agent_high(self.normal)?;
        }
        Ok(())
    }
}

fn agent_high() -> Result<u64, Error> {
    let value = fs::read_to_string(AGENT_HIGH)?;
    value
        .trim()
        .parse()
        .map_err(|_| Error::Refused("The managed agent memory limit is unavailable."))
}

fn set_agent_high(bytes: u64) -> Result<(), Error> {
    // Let the agent's next allocation do reclaim instead of stalling this controller.
    OpenOptions::new()
        .write(true)
        .custom_flags(nix::libc::O_NOFOLLOW | nix::libc::O_NONBLOCK)
        .open(AGENT_HIGH)?
        .write_all(bytes.to_string().as_bytes())?;
    Ok(())
}

fn some_avg10(psi: &str) -> Result<f64, Error> {
    psi.lines()
        .find_map(|line| line.strip_prefix("some "))
        .and_then(|record| {
            record
                .split_whitespace()
                .find_map(|entry| entry.strip_prefix("avg10="))
        })
        .and_then(|value| value.parse().ok())
        .filter(|value| (0.0..=100.0).contains(value))
        .ok_or(Error::Refused(
            "Hosting memory pressure could not be observed.",
        ))
}

fn recovered(quiet_since: &mut Option<Instant>, now: Instant, pressure: f64) -> bool {
    if pressure >= RECOVERY_PERCENT {
        *quiet_since = None;
        return false;
    }
    now.duration_since(*quiet_since.get_or_insert(now)) >= RECOVERY_TIME
}

#[cfg(test)]
mod tests;
