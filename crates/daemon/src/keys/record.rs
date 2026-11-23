//! The stored form of the key cache (K-E5 Section 2.1); a record that does not read says only that (LOG-007).

use chrono::{DateTime, Utc};
use daemon_core::wire::{FetchedKey, MAX_KEYS, MAX_VALUE_BYTES};
use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

use super::cache::KeyCache;

/// Each key at its longest value, with room for its name and the punctuation.
pub const RECORD_LIMIT: usize = 64 + MAX_KEYS * (256 + MAX_VALUE_BYTES / 3 * 4 + 4);

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Written<'a> {
    answered_at: Option<DateTime<Utc>>,
    keys: Vec<&'a FetchedKey>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Read {
    answered_at: Option<DateTime<Utc>>,
    keys: Vec<FetchedKey>,
}

/// The buffer has room for the record from the start, so it never moves and its wipe covers every value.
///
/// # Panics
/// Never: names, versions, a time and base64 text always serialise.
#[must_use]
pub fn render(cache: &KeyCache) -> Zeroizing<Vec<u8>> {
    let written = Written {
        answered_at: cache.answered_at(),
        keys: cache.keys().collect(),
    };
    let room = 128
        + written
            .keys
            .iter()
            .map(|key| 64 + key.name.len() + key.value.expose().len() / 3 * 4 + 4)
            .sum::<usize>();
    let mut bytes = Zeroizing::new(Vec::with_capacity(room));
    serde_json::to_writer(&mut *bytes, &written).expect("a key cache record serialises");
    bytes
}

#[must_use]
pub fn parse(bytes: &[u8]) -> Option<KeyCache> {
    let read: Read = serde_json::from_slice(bytes).ok()?;
    (read.keys.len() <= MAX_KEYS).then(|| KeyCache::of(read.keys, read.answered_at))
}

#[cfg(test)]
mod tests {
    use chrono::TimeDelta;
    use daemon_core::wire::{KeyValue, KeyVersion};

    use super::*;

    #[test]
    fn r_147_a_record_reads_back_as_the_cache_it_was() {
        let at = DateTime::UNIX_EPOCH + TimeDelta::days(3);
        let value = KeyValue::new(Zeroizing::new(vec![0, 1, 2, 255])).unwrap();
        let key = FetchedKey {
            name: "cloudflare.dns".to_owned(),
            version: 4,
            value,
        };
        let cache = KeyCache::of(vec![key], Some(at));
        let bytes = render(&cache);
        let read = parse(&bytes).unwrap();
        let held = [KeyVersion {
            name: "cloudflare.dns".to_owned(),
            version: 4,
        }];
        assert_eq!(read.held(), held);
        assert_eq!(read.answered_at(), Some(at));
        assert_eq!(
            read.value("cloudflare.dns").unwrap().expose(),
            [0, 1, 2, 255]
        );
    }

    #[test]
    fn r_147_a_record_that_does_not_read_is_no_cache() {
        assert!(parse(b"{").is_none());
        assert!(
            parse(br#"{"answeredAt":null,"keys":[{"name":"a","version":1,"value":"!"}]}"#)
                .is_none()
        );
    }
}
