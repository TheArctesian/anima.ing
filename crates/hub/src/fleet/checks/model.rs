use std::{fmt, net::IpAddr};

use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use chrono::{DateTime, Utc};

use super::{
    config::{
        CheckConfig, CheckType, HttpConfig, HttpInput, HttpSecrets, SnmpConfig, SnmpInput,
        SnmpSecrets,
    },
    evaluation::{CheckSettings, CheckState},
    probe::ProbeResult,
};
use crate::{
    fleet::{credentials::Vault, devices::Device},
    ids::{CheckId, DeviceId},
    status::Status,
};

pub use crate::api_errors::FieldError;

/// HTTP and SNMP secrets are never part of a stored check (LOG-007).
#[derive(Clone, Debug, PartialEq)]
pub struct Check {
    pub id: CheckId,
    pub device: DeviceId,
    pub parent: Option<CheckId>,
    pub name: String,
    pub config: CheckConfig,
    pub settings: CheckSettings,
    pub paused: bool,
    /// The stored state of the last result; read the shown status through the status rules.
    pub state: CheckState,
    pub last_beat_at: Option<DateTime<Utc>>,
    /// Leaf certificate expiry from the last TLS or HTTPS result that had one (CHK-007).
    pub cert_not_after: Option<DateTime<Utc>>,
    /// Registration expiry from the last RDAP reply that gave one (D-29).
    pub domain_not_after: Option<DateTime<Utc>>,
    pub resolved_ip: Option<IpAddr>,
    pub push_token: Option<PushToken>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Check {
    #[must_use]
    pub const fn kind(&self) -> CheckType {
        self.config.kind()
    }

    /// Whole days to the certificate's expiry, negative once it expired; `None` unless a TLS or
    /// HTTPS check has seen a certificate (CHK-007).
    #[must_use]
    pub fn certificate_days_left(&self, now: DateTime<Utc>) -> Option<i32> {
        let secure = match &self.config {
            CheckConfig::Tls { .. } => true,
            CheckConfig::Http(http) => {
                reqwest::Url::parse(&http.url).is_ok_and(|url| url.scheme() == "https")
            }
            _ => false,
        };
        let not_after = self.cert_not_after?;
        secure.then(|| whole_days(not_after, now))
    }

    /// Whole days to the registration expiry, negative once it expired; `None` unless a domain
    /// expiry check got a date (D-29). A later reply without a date keeps the last date.
    #[must_use]
    pub fn domain_days_left(&self, now: DateTime<Utc>) -> Option<i32> {
        let not_after = self.domain_not_after?;
        matches!(self.config, CheckConfig::DomainExpiry { .. }).then(|| whole_days(not_after, now))
    }
}

#[must_use]
pub(in crate::fleet::checks) fn whole_days(end: DateTime<Utc>, now: DateTime<Utc>) -> i32 {
    let days = (end - now).num_seconds().div_euclid(86_400);
    i32::try_from(days).unwrap_or(if days < 0 { i32::MIN } else { i32::MAX })
}

/// One stored result of a check (CHK-003).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Heartbeat {
    pub at: DateTime<Utc>,
    pub status: Status,
    pub response_time_ms: Option<u32>,
    pub message: Option<String>,
}

/// The only authentication of the push route, so `Debug` never prints it (LOG-007).
#[derive(Clone, PartialEq, Eq)]
pub struct PushToken(String);

impl PushToken {
    #[must_use]
    pub fn from_random(bytes: &[u8; 32]) -> Self {
        Self(URL_SAFE_NO_PAD.encode(bytes))
    }

    #[must_use]
    pub fn expose(&self) -> &str {
        &self.0
    }

    pub(super) const fn stored(text: String) -> Self {
        Self(text)
    }
}

impl fmt::Debug for PushToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("PushToken(..)")
    }
}

pub enum Definition {
    /// Every type except HTTP, SNMP and PUSH.
    Config(CheckConfig),
    /// The HTTP check with its secrets in hand; they are sealed apart from `config`.
    Http(HttpInput),
    /// The SNMP check with its secrets in hand; they are sealed apart from `config`.
    Snmp(SnmpInput),
    Push(PushToken),
}

impl Definition {
    #[must_use]
    pub const fn kind(&self) -> CheckType {
        match self {
            Self::Config(config) => config.kind(),
            Self::Http(_) => CheckType::Http,
            Self::Snmp(_) => CheckType::Snmp,
            Self::Push(_) => CheckType::Push,
        }
    }
}

/// What a new check starts with (D-26); bounds as `checks.checks`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CheckDefaults {
    pub interval_seconds: u32,
    pub retries: u8,
    pub retry_interval_seconds: u32,
    pub degraded_threshold_ms: u32,
}

impl CheckDefaults {
    #[must_use]
    pub const fn settings(self) -> CheckSettings {
        CheckSettings {
            interval_seconds: self.interval_seconds,
            retries: self.retries,
            retry_interval_seconds: self.retry_interval_seconds,
            degraded_threshold_ms: self.degraded_threshold_ms,
            invert: false,
        }
    }
}

/// SNMP secrets are sealed for the host of the check's device as the operator sees it (SEC-010).
#[derive(Clone, Copy)]
pub struct Sealer<'a> {
    pub vault: &'a Vault,
    pub host: &'a str,
}

pub struct NewCheck {
    pub device: DeviceId,
    /// A group check on the same device.
    pub parent: Option<CheckId>,
    pub name: String,
    pub definition: Definition,
    pub settings: CheckSettings,
}

/// `None` keeps a field. Only the hub's method repair changes a check's type (CHK-013).
///
/// An empty HTTP secret keeps the stored one. An SNMP secret is kept only while the version,
/// the user name and the device host are the ones it was entered for (SEC-005, SEC-010).
#[derive(Default)]
pub struct CheckChange {
    /// `Some(None)` removes the check from its group.
    pub parent: Option<Option<CheckId>>,
    pub name: Option<String>,
    pub definition: Option<Definition>,
    pub interval_seconds: Option<u32>,
    pub retries: Option<u8>,
    pub retry_interval_seconds: Option<u32>,
    pub degraded_threshold_ms: Option<u32>,
    pub invert: Option<bool>,
    /// True for a group's change; every other change is an operator edit and marks the check (CHK-013).
    pub regrouping: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Missing {
    Device(DeviceId),
    Check(CheckId),
}

impl fmt::Display for Missing {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Device(id) => write!(f, "device {id}"),
            Self::Check(id) => write!(f, "check {id}"),
        }
    }
}

/// One result to record: the heartbeat (CHK-003) and the check's new state.
#[derive(Clone, Debug, PartialEq)]
pub struct Beat {
    pub at: DateTime<Utc>,
    pub status: Status,
    pub response_time_ms: Option<u32>,
    /// Cleaned again when written: at most 512 characters, no control characters.
    pub message: Option<String>,
    pub state: CheckState,
    /// Kept on the check only when the probe reported one.
    pub resolved_ip: Option<IpAddr>,
    pub cert_not_after: Option<DateTime<Utc>>,
    /// Kept on the check only when the probe reported one.
    pub domain_not_after: Option<DateTime<Utc>>,
}

impl Beat {
    /// The response time is kept only for an Up or Degraded beat of a probe that succeeded, so
    /// the average describes answered probes.
    #[must_use]
    pub fn of(
        probe: ProbeResult,
        (state, status): (CheckState, Status),
        at: DateTime<Utc>,
    ) -> Self {
        let answered = probe.ok && matches!(status, Status::Up | Status::Degraded);
        let millis =
            |time: std::time::Duration| u32::try_from(time.as_millis()).unwrap_or(u32::MAX);
        Self {
            at,
            status,
            response_time_ms: probe.response_time.filter(|_| answered).map(millis),
            message: probe.message,
            state,
            resolved_ip: probe.resolved_ip,
            cert_not_after: probe.cert_not_after,
            domain_not_after: probe.domain_not_after,
        }
    }
}

/// The evaluation runs on the stored state and settings inside the write, so a beat, pause or
/// change that landed during the probe is not lost.
#[derive(Clone, Copy, Debug)]
pub(in crate::fleet::checks) struct Attempt<'a> {
    pub(in crate::fleet::checks) result: &'a ProbeResult,
    /// It only turns a Down beat into a Maintenance beat (CHK-006).
    pub(in crate::fleet::checks) in_maintenance: bool,
    pub(in crate::fleet::checks) at: DateTime<Utc>,
    /// The domain a domain expiry probe asked about. Its expiry date is kept only while the
    /// check still names that domain (D-29).
    pub(in crate::fleet::checks) domain: Option<&'a str>,
    /// The check's device, as the attempt read it: a notification names it (D-16).
    pub(in crate::fleet::checks) device: &'a Device,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[must_use]
pub enum Recorded {
    Written,
    /// Nothing was written: the check is paused, or the beat is not newer than its last beat.
    Skipped,
}

/// Read together with its secrets, so they go only where this configuration points (SEC-005).
pub struct HttpSetup {
    pub config: HttpConfig,
    pub secrets: Option<HttpSecrets>,
}

/// Read together with its secrets, so they go only to the version and user they were given
/// for (SEC-005).
pub struct SnmpSetup {
    pub config: SnmpConfig,
    pub secrets: Option<SnmpSecrets>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Due {
    pub id: CheckId,
    pub device: DeviceId,
    /// The group the check belongs to; maintenance on the group covers it (CHK-006).
    pub parent: Option<CheckId>,
    /// `None` when the stored configuration cannot be read; the probe then fails as a bad
    /// configuration instead of stopping the schedule.
    pub config: Option<CheckConfig>,
    pub settings: CheckSettings,
    pub state: CheckState,
    /// For a domain expiry check, its last RDAP query (R-102).
    pub last_beat_at: Option<DateTime<Utc>>,
}
