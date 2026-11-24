//! Inside or outside a place, from the location samples (K-E6 Section 4; REM-006). Pure.

use chrono::{DateTime, Utc};

use super::model::PlaceChange;
use crate::{
    engine::{
        locations::{RULE, Sample, inside},
        places::Place,
    },
    ids::{PlaceId, SampleId},
};

/// A place with no row in `reminders.place_state` is outside.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlaceState {
    pub place: PlaceId,
    pub inside: bool,
    pub changed_at: DateTime<Utc>,
    pub sample: SampleId,
}

/// A sample with no time, position or accuracy, or one worse than 200 m, changes nothing
/// (R-145); a sample older than a state's leaves it, so a late import makes no past arrival.
#[must_use]
pub fn place_changes(
    states: &[PlaceState],
    samples: &[Sample],
    places: &[Place],
) -> Vec<PlaceChange> {
    let mut ordered: Vec<(DateTime<Utc>, (f64, f64), SampleId)> = samples
        .iter()
        .filter(|sample| {
            sample
                .position
                .accuracy_m
                .is_some_and(|accuracy| accuracy <= RULE.worst_accuracy_m)
        })
        .filter_map(|sample| {
            let point = sample.position.latitude.zip(sample.position.longitude)?;
            Some((sample.observed_at?, point, sample.id))
        })
        .collect();
    ordered.sort_by_key(|(at, _, id)| (*at, uuid::Uuid::from(*id)));
    let mut current: Vec<(PlaceId, bool, DateTime<Utc>)> = states
        .iter()
        .map(|state| (state.place, state.inside, state.changed_at))
        .collect();
    let mut changes = Vec::new();
    for (at, point, sample) in ordered {
        for place in places {
            let Some(centre) = place.centre.filter(|_| place.parent.is_none()) else {
                continue;
            };
            if place.merged_into.is_some() {
                continue;
            }
            let now_inside = inside(&centre, point);
            let known = current.iter_mut().find(|(id, ..)| *id == place.id);
            let (was_inside, since) = known
                .as_ref()
                .map_or((false, None), |(_, inside, at)| (*inside, Some(*at)));
            if now_inside == was_inside || since.is_some_and(|since| at < since) {
                continue;
            }
            match known {
                Some(state) => (state.1, state.2) = (now_inside, at),
                None => current.push((place.id, now_inside, at)),
            }
            changes.push(PlaceChange {
                place: place.id,
                inside: now_inside,
                sample,
                observed_at: at,
            });
        }
    }
    changes
}
