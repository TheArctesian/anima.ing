//! Domain types of web interfaces (WEB-001, WEB-002, WEB-008).

use std::fmt;

use chrono::{DateTime, Utc};

use crate::{
    fingerprint::CertificateFingerprint,
    ids::{CheckId, DeviceId, OperatorId, WebInterfaceId},
};

pub use crate::api_errors::FieldError;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OpenIn {
    AnimaTab,
    SystemBrowser,
}

impl OpenIn {
    pub(super) const fn as_str(self) -> &'static str {
        match self {
            Self::AnimaTab => "anima_tab",
            Self::SystemBrowser => "system_browser",
        }
    }

    pub(super) fn from_stored(text: &str) -> Option<Self> {
        match text {
            "anima_tab" => Some(Self::AnimaTab),
            "system_browser" => Some(Self::SystemBrowser),
            _ => None,
        }
    }
}

/// The fingerprint is stored and shown as 64 lower-case hex digits (D-5).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pin {
    pub fingerprint: CertificateFingerprint,
    pub at: DateTime<Utc>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WebInterface {
    pub id: WebInterfaceId,
    pub device: DeviceId,
    pub label: String,
    pub url: String,
    pub open_in: OpenIn,
    pub pin: Option<Pin>,
}

/// An HTTP check's address offered as an interface (WEB-002).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Suggestion {
    pub check: CheckId,
    pub url: String,
    pub label: String,
}

/// The label defaults to the URL's host.
#[derive(Clone, Debug)]
pub struct WebInterfaceInput {
    pub label: Option<String>,
    pub url: String,
    pub open_in: OpenIn,
}

/// The fingerprint is as the operator typed it.
#[derive(Clone, Copy, Debug)]
pub struct Acceptance<'a> {
    pub fingerprint: &'a str,
    pub operator: OperatorId,
    pub at: DateTime<Utc>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Updated {
    pub before: WebInterface,
    pub after: WebInterface,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Accepted {
    pub interface: WebInterface,
    pub created: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Missing {
    Interface(WebInterfaceId),
    Device(DeviceId),
    Check(CheckId),
}

impl fmt::Display for Missing {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Interface(id) => write!(f, "web interface {id}"),
            Self::Device(id) => write!(f, "device {id}"),
            Self::Check(id) => write!(f, "check {id}"),
        }
    }
}
