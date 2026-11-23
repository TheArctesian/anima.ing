//! The enrolment and renewal bodies (K8 Sections 3 and 5). The only serde paths for an
//! [`EnrolmentCode`] or an [`DaemonToken`] are the fields here.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{DaemonToken, EnrolmentCode, secrets};

/// `POST /daemon/v1/enrol` (K8 Section 3).
#[derive(Serialize, Deserialize)]
pub struct EnrolRequest {
    #[serde(
        serialize_with = "secrets::write_code",
        deserialize_with = "secrets::read_code"
    )]
    pub code: EnrolmentCode,
}

/// The hub's device rule takes 1 to this many characters (K8 Section 3, DEV-010).
pub const DEVICE_NAME_CHARS: usize = 64;

#[must_use]
pub fn is_device_name(name: &str) -> bool {
    (1..=DEVICE_NAME_CHARS).contains(&name.chars().count()) && !name.chars().any(char::is_control)
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Enrolled {
    pub device: Uuid,
    pub device_name: String,
    #[serde(
        serialize_with = "secrets::write_token",
        deserialize_with = "secrets::read_token"
    )]
    pub token: DaemonToken,
    #[serde(with = "super::times")]
    pub expires_at: DateTime<Utc>,
}

/// `POST /daemon/v1/renew`: the body is `{}`; the token is the bearer (K8 Section 5).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RenewRequest {}

/// The next token replaces the current one on its first use.
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Renewed {
    #[serde(
        serialize_with = "secrets::write_token",
        deserialize_with = "secrets::read_token"
    )]
    pub token: DaemonToken,
    #[serde(with = "super::times")]
    pub expires_at: DateTime<Utc>,
}
