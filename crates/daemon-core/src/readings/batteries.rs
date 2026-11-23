//! Readers of the batteries that power a device (MET-032, CAP-004). The text is the device's:
//! nothing here panics on it, and a value that does not read is left out rather than guessed.

use super::{label, lines, number};

pub const MAX_BATTERIES: usize = 8;

pub const POWER_SUPPLY: &str = "/sys/class/power_supply";

pub const SUPPLY_FILES: [&str; 11] = [
    "type",
    "scope",
    "present",
    "capacity",
    "status",
    "time_to_empty_now",
    "time_to_full_now",
    "energy_full",
    "energy_full_design",
    "charge_full",
    "charge_full_design",
];

pub const PMSET_ARGS: [&str; 2] = ["-g", "batt"];

/// The hub keeps it as a 32-bit integer (K8 Section 4).
const MAX_SECONDS_LEFT: u32 = i32::MAX.unsigned_abs();

/// A UPS that `pmset` lists is another device's battery.
const INTERNAL_BATTERY: &str = "InternalBattery";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BatteryState {
    Charging,
    Discharging,
    Full,
    NotCharging,
    Unknown,
}

impl BatteryState {
    /// The text kept in `metrics.batteries.state` and sent in a batch's `batteries`.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Charging => "charging",
            Self::Discharging => "discharging",
            Self::Full => "full",
            Self::NotCharging => "not_charging",
            Self::Unknown => "unknown",
        }
    }

    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        [
            Self::Charging,
            Self::Discharging,
            Self::Full,
            Self::NotCharging,
            Self::Unknown,
        ]
        .into_iter()
        .find(|state| state.as_str() == text)
    }

    fn of_status(text: &str) -> Self {
        match text {
            "Charging" => Self::Charging,
            "Discharging" => Self::Discharging,
            "Full" => Self::Full,
            "Not charging" => Self::NotCharging,
            _ => Self::Unknown,
        }
    }

    fn of_pmset(text: &str) -> Self {
        match text {
            "charging" | "finishing charge" => Self::Charging,
            "discharging" => Self::Discharging,
            "charged" => Self::Full,
            "AC attached" => Self::NotCharging,
            _ => Self::Unknown,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Battery {
    /// The name the system gives it (`BAT0`, `InternalBattery-0`), a series label.
    pub name: String,
    pub percent: Option<f64>,
    pub state: BatteryState,
    /// To empty while it discharges, to full while it charges.
    pub seconds_left: Option<u32>,
    /// The capacity when full against the capacity of its design, 0 to 100.
    pub health_percent: Option<u8>,
}

#[must_use]
pub fn batteries(found: impl IntoIterator<Item = Battery>) -> Vec<Battery> {
    let mut kept: Vec<Battery> = found.into_iter().collect();
    kept.sort_by(|a, b| a.name.cmp(&b.name));
    kept.dedup_by(|later, earlier| later.name == earlier.name);
    kept.truncate(MAX_BATTERIES);
    kept
}

/// `files` holds each of [`SUPPLY_FILES`] that could be read, with its text.
#[must_use]
pub fn linux_battery(name: &str, files: &[(&str, &str)]) -> Option<Battery> {
    let file = |wanted: &str| {
        files
            .iter()
            .find(|(file, _)| *file == wanted)
            .map(|(_, text)| text.trim())
    };
    let is_battery = file("type") == Some("Battery")
        && file("scope") != Some("Device")
        && file("present") != Some("0");
    if !is_battery {
        return None;
    }
    let state = BatteryState::of_status(file("status").unwrap_or_default());
    let time_file = match state {
        BatteryState::Discharging => Some("time_to_empty_now"),
        BatteryState::Charging => Some("time_to_full_now"),
        _ => None,
    };
    let health = health_percent(file("energy_full"), file("energy_full_design"))
        .or_else(|| health_percent(file("charge_full"), file("charge_full_design")));
    Some(Battery {
        name: label(name)?.to_owned(),
        percent: file("capacity").and_then(percent),
        state,
        seconds_left: time_file.and_then(file).and_then(seconds_left),
        health_percent: health,
    })
}

#[must_use]
pub fn pmset(output: &[u8]) -> Option<Vec<Battery>> {
    let mut rows = lines(output);
    if !rows.next()?.starts_with("Now drawing from") {
        return None;
    }
    Some(batteries(rows.filter_map(pmset_battery)))
}

/// One line such as ` -InternalBattery-0 (id=1234)\t85%; discharging; 4:12 remaining present:
/// true`.
fn pmset_battery(line: &str) -> Option<Battery> {
    let rest = line.trim().strip_prefix('-')?;
    let (head, tail) = rest.split_once('\t').or_else(|| rest.split_once(") "))?;
    let name = head
        .split_once(" (id=")
        .map_or(head, |(name, _)| name)
        .trim();
    if !name.starts_with(INTERNAL_BATTERY) {
        return None;
    }
    let mut parts = tail.split(';').map(str::trim);
    let charge = parts.next()?;
    let state = BatteryState::of_pmset(parts.next()?);
    let remainder = parts.next().unwrap_or_default();
    if remainder.contains("present: false") {
        return None;
    }
    let timed = matches!(state, BatteryState::Charging | BatteryState::Discharging);
    Some(Battery {
        name: label(name)?.to_owned(),
        percent: charge.strip_suffix('%').and_then(percent),
        state,
        seconds_left: timed.then(|| clock_left(remainder)).flatten(),
        health_percent: None,
    })
}

fn clock_left(text: &str) -> Option<u32> {
    let (clock, after) = text.split_once(' ')?;
    if !after.starts_with("remaining") {
        return None;
    }
    let (hours, minutes) = clock.split_once(':')?;
    let hours: u32 = hours.parse().ok()?;
    let minutes: u32 = minutes.parse().ok().filter(|minutes| *minutes < 60)?;
    seconds_left(
        &hours
            .checked_mul(3600)?
            .checked_add(minutes * 60)?
            .to_string(),
    )
}

fn percent(text: &str) -> Option<f64> {
    number(text.trim()).filter(|value| *value <= 100.0)
}

/// 0 is what a driver writes when it has no estimate.
fn seconds_left(text: &str) -> Option<u32> {
    text.trim()
        .parse::<u32>()
        .ok()
        .filter(|seconds| (1..=MAX_SECONDS_LEFT).contains(seconds))
}

fn health_percent(full: Option<&str>, design: Option<&str>) -> Option<u8> {
    let full: u64 = full?.parse().ok()?;
    let design: u64 = design?.parse().ok().filter(|design| *design > 0)?;
    let share = u128::from(full) * 100 / u128::from(design);
    u8::try_from(share.min(100)).ok()
}
