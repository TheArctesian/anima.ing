//! A replacement holder may replay or complete only work originally sent to its transport.

use crate::threads::link::Origin;
use std::collections::BTreeMap;

pub(super) struct Outstanding<T>(BTreeMap<u64, (Origin, T)>);

impl<T> Outstanding<T> {
    pub(super) const fn new() -> Self {
        Self(BTreeMap::new())
    }

    pub(super) fn insert(&mut self, id: u64, origin: Origin, value: T) {
        self.0.insert(id, (origin, value));
    }

    pub(super) fn for_holder(&self, origin: Origin) -> impl Iterator<Item = (u64, &T)> {
        self.0
            .iter()
            .filter_map(move |(id, (at, value))| (*at == origin).then_some((*id, value)))
    }

    pub(super) fn foreign(&self, id: u64, origin: Origin) -> bool {
        self.0.get(&id).is_some_and(|(at, _)| *at != origin)
    }

    pub(super) fn take(&mut self, id: u64, origin: Origin) -> Option<T> {
        if self.foreign(id, origin) {
            return None;
        }
        self.0.remove(&id).map(|(_, value)| value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustls::pki_types::CertificateDer;

    #[test]
    fn only_the_original_enrollment_replays_or_acknowledges_a_provider_command() {
        let first = Origin::of(Some(&CertificateDer::from(&b"first-root"[..])));
        let renewal = Origin::of(Some(&CertificateDer::from(&b"first-root"[..])));
        let fresh = Origin::of(Some(&CertificateDer::from(&b"fresh-root"[..])));
        let mut pending = Outstanding::new();
        pending.insert(31, first, "provider-write");
        for other in [fresh, Origin::of(None)] {
            assert_eq!(pending.for_holder(other).count(), 0);
            assert!(pending.take(31, other).is_none());
        }
        assert_eq!(
            pending.for_holder(renewal).collect::<Vec<_>>(),
            vec![(31, &"provider-write")]
        );
        assert_eq!(pending.take(31, renewal), Some("provider-write"));
    }
}
