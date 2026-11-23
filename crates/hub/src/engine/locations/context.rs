//! The context of a transfer event (K-E4 Section 12); the evidence for the operator's position is not stored.
//! The evidence is a pure function of the event time and the samples near it, so a late sample gives the same answer and a newer position never replaces it (R-145).

use std::time::Duration;

use chrono::{DateTime, TimeDelta, Utc};

use super::model::Sample;
use crate::{
    engine::places::{Centre, Place},
    fleet::events,
    ids::{DeviceId, EventId, PlaceId, SampleId, SiteId},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    /// From the computer of the client to the device.
    Upload,
    /// From the device to the computer of the client.
    Download,
}

impl Direction {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Upload => "upload",
            Self::Download => "download",
        }
    }

    pub(super) fn parse(text: &str) -> Option<Self> {
        [Self::Upload, Self::Download]
            .into_iter()
            .find(|direction| direction.as_str() == text)
    }
}

/// From the token of the call: that a device is of the operator is not evidence that the operator used it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActorKind {
    Human,
    /// An AI agent or an API key that declared one.
    Agent,
    System,
}

impl ActorKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Human => "human",
            Self::Agent => "agent",
            Self::System => "system",
        }
    }

    pub(super) fn parse(text: &str) -> Option<Self> {
        [Self::Human, Self::Agent, Self::System]
            .into_iter()
            .find(|kind| kind.as_str() == text)
    }
}

impl From<events::ActorKind> for ActorKind {
    fn from(kind: events::ActorKind) -> Self {
        match kind {
            events::ActorKind::User => Self::Human,
            events::ActorKind::Agent => Self::Agent,
            events::ActorKind::Hub => Self::System,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Actor {
    pub kind: ActorKind,
    pub name: Option<String>,
    pub client_device: Option<DeviceId>,
}

/// Each is `None` when the hub did not know it: unknown, not the hub.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct End {
    pub device: Option<DeviceId>,
    pub site: Option<SiteId>,
    pub place: Option<PlaceId>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Stored {
    pub event: EventId,
    pub occurred_at: DateTime<Utc>,
    pub direction: Direction,
    pub source: End,
    pub destination: End,
    pub actor: Actor,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Basis {
    /// A sample not more than [`MatchRule::at_the_time`] from the event.
    AtTheTime,
    /// No such sample; the nearest one not more than [`MatchRule::nearest`] from the event.
    NearestInferred,
    Unknown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OperatorEvidence {
    pub basis: Basis,
    pub sample: Option<SampleId>,
    /// The time of the sample less the time of the event, in seconds.
    pub offset_seconds: Option<i64>,
    /// The sample has no accuracy, or an accuracy worse than the rule permits.
    pub uncertain: bool,
    /// Given only for a sample at the time that is not uncertain.
    pub place: Option<PlaceId>,
}

impl OperatorEvidence {
    const UNKNOWN: Self = Self {
        basis: Basis::Unknown,
        sample: None,
        offset_seconds: None,
        uncertain: false,
        place: None,
    };
}

/// The limits of a match (R-145).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MatchRule {
    pub at_the_time: Duration,
    pub nearest: Duration,
    pub worst_accuracy_m: f64,
}

pub const RULE: MatchRule = MatchRule {
    at_the_time: Duration::from_secs(120),
    nearest: Duration::from_secs(600),
    worst_accuracy_m: 200.0,
};

const EARTH_RADIUS_M: f64 = 6_371_008.8;

/// Haversine distance in metres.
fn distance_m((lat_a, lon_a): (f64, f64), (lat_b, lon_b): (f64, f64)) -> f64 {
    let (phi_a, phi_b) = (lat_a.to_radians(), lat_b.to_radians());
    let half_lat = ((lat_b - lat_a).to_radians() / 2.0).sin();
    let half_lon = ((lon_b - lon_a).to_radians() / 2.0).sin();
    let h = half_lat * half_lat + phi_a.cos() * phi_b.cos() * half_lon * half_lon;
    2.0 * EARTH_RADIUS_M * h.sqrt().min(1.0).asin()
}

#[must_use]
pub fn inside(centre: &Centre, point: (f64, f64)) -> bool {
    distance_m(point, (centre.latitude, centre.longitude)) <= f64::from(centre.radius_m)
}

/// The nearest centre wins when more than one top-level place contains `point`.
fn place_at(point: (f64, f64), places: &[Place]) -> Option<PlaceId> {
    places
        .iter()
        .filter(|place| place.parent.is_none() && place.merged_into.is_none())
        .filter_map(|place| {
            let centre = place.centre?;
            let distance = distance_m(point, (centre.latitude, centre.longitude));
            (distance <= f64::from(centre.radius_m)).then_some((distance, place.id))
        })
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, id)| id)
}

/// The nearest sample decides, the earlier on a tie; a sample with no time never matches, and an inferred or uncertain one is never a confirmed place.
#[must_use]
pub fn operator_evidence(
    event_at: DateTime<Utc>,
    near: &[Sample],
    places: &[Place],
    rule: &MatchRule,
) -> OperatorEvidence {
    let nearest = near
        .iter()
        .filter_map(|sample| Some((sample, sample.observed_at? - event_at)))
        .min_by_key(|(_, offset)| (offset.abs(), *offset));
    let Some((sample, offset)) = nearest else {
        return OperatorEvidence::UNKNOWN;
    };
    let within =
        |limit: Duration| TimeDelta::from_std(limit).is_ok_and(|limit| offset.abs() <= limit);
    let basis = if within(rule.at_the_time) {
        Basis::AtTheTime
    } else if within(rule.nearest) {
        Basis::NearestInferred
    } else {
        return OperatorEvidence::UNKNOWN;
    };
    let position = sample.position;
    let point = position.latitude.zip(position.longitude);
    let uncertain = point.is_none()
        || position
            .accuracy_m
            .is_none_or(|accuracy| accuracy > rule.worst_accuracy_m);
    let confirmed = basis == Basis::AtTheTime && !uncertain;
    OperatorEvidence {
        basis,
        sample: Some(sample.id),
        offset_seconds: Some(offset.num_seconds()),
        uncertain,
        place: point
            .filter(|_| confirmed)
            .and_then(|point| place_at(point, places)),
    }
}
