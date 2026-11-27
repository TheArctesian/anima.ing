//! This local signing key has the access list of the daemon's login Keychain entry.
//! Fresh enrollment rotates it. Ordinary K8 bearer renewal retains it for a surviving holder.

use serde::Deserialize;
use zeroize::{Zeroize as _, Zeroizing};

#[derive(Deserialize)]
struct Record {
    hub: String,
    #[serde(rename = "holderAuthority")]
    key: Option<[u8; 32]>,
}

impl Drop for Record {
    fn drop(&mut self) {
        self.key.zeroize();
    }
}

pub(super) fn at(bytes: &[u8], hub: &str) -> Option<Zeroizing<[u8; 32]>> {
    let stored: Record = serde_json::from_slice(bytes).ok()?;
    (stored.hub == hub).then_some(Zeroizing::new(stored.key?))
}

pub(super) fn record(text: &str, previous: Option<&[u8]>) -> Result<Zeroizing<String>, String> {
    let new: Record = serde_json::from_str(text).map_err(|_| "unreadable credential".to_owned())?;
    let key = if let Some(key) = previous.and_then(|old| at(old, &new.hub)) {
        key
    } else {
        let mut key = Zeroizing::new([0; 32]);
        getrandom::fill(&mut *key).map_err(|_| "holder credential unavailable".to_owned())?;
        key
    };
    let mut out = Zeroizing::new(String::with_capacity(text.len() + 160));
    out.push_str(
        text.strip_suffix('}')
            .ok_or_else(|| "unreadable credential".to_owned())?,
    );
    out.push_str(",\"holderAuthority\":");
    let encoded = Zeroizing::new(
        serde_json::to_string(&*key).map_err(|_| "holder credential unavailable".to_owned())?,
    );
    out.push_str(&encoded);
    out.push('}');
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renewal_keeps_the_holder_and_fresh_enrollment_replaces_it() {
        let hub = "https://hub.example";
        let original = r#"{"hub":"https://hub.example","token":"synthetic-first"}"#;
        let new = r#"{"hub":"https://hub.example","token":"synthetic-renewal"}"#;
        let initial = record(original, None).unwrap();
        let renewed = record(new, Some(initial.as_bytes())).unwrap();
        let reset = record(new, None).unwrap();
        let first_key = at(initial.as_bytes(), hub).unwrap();
        assert_eq!(*at(renewed.as_bytes(), hub).unwrap(), *first_key);
        assert_ne!(*at(reset.as_bytes(), hub).unwrap(), *first_key);
        assert!(at(renewed.as_bytes(), "https://elsewhere.example").is_none());
        assert!(renewed.contains("synthetic-renewal"));
    }
}
