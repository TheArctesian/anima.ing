//! Times in K8 bodies: RFC 3339 UTC with `Z` and whole seconds (K8 Section 1).

use chrono::{DateTime, NaiveDateTime, SecondsFormat, Utc};
use serde::{Deserialize as _, Deserializer, Serializer, de::Error as _};

const FORM: &[u8; 20] = b"0000-00-00T00:00:00Z";

pub(super) fn serialize<S: Serializer>(at: &DateTime<Utc>, out: S) -> Result<S::Ok, S::Error> {
    out.serialize_str(&at.to_rfc3339_opts(SecondsFormat::Secs, true))
}

/// Reads exactly the form K8 Section 1 writes, else the hub answers `MALFORMED`; the error never holds the text.
pub(super) fn deserialize<'de, D: Deserializer<'de>>(input: D) -> Result<DateTime<Utc>, D::Error> {
    let text = String::deserialize(input)?;
    parse(&text).ok_or_else(|| D::Error::custom("not a K8 time"))
}

fn parse(text: &str) -> Option<DateTime<Utc>> {
    if !shaped(text, FORM) {
        return None;
    }
    let at = NaiveDateTime::parse_from_str(&text[..19], "%Y-%m-%dT%H:%M:%S")
        .ok()?
        .and_utc();
    (at.timestamp_subsec_nanos() == 0).then_some(at)
}

/// A digit where `form` has `0`, the same byte elsewhere.
fn shaped(text: &str, form: &[u8]) -> bool {
    text.len() == form.len()
        && text.bytes().zip(form).all(|(byte, form)| match form {
            b'0' => byte.is_ascii_digit(),
            _ => byte == *form,
        })
}

/// UTC days in K8 bodies: `YYYY-MM-DD`, a real date (K8 Section 4, `agentUsage`).
pub(super) mod days {
    use chrono::NaiveDate;
    use serde::{Deserialize as _, Deserializer, Serializer, de::Error as _};

    const FORM: &[u8; 10] = b"0000-00-00";

    #[expect(
        clippy::trivially_copy_pass_by_ref,
        reason = "serde's `serialize_with` passes a reference"
    )]
    pub(in super::super) fn serialize<S: Serializer>(
        day: &NaiveDate,
        out: S,
    ) -> Result<S::Ok, S::Error> {
        out.serialize_str(&day.format("%Y-%m-%d").to_string())
    }

    pub(in super::super) fn deserialize<'de, D: Deserializer<'de>>(
        input: D,
    ) -> Result<NaiveDate, D::Error> {
        let text = String::deserialize(input)?;
        super::shaped(&text, FORM)
            .then(|| NaiveDate::parse_from_str(&text, "%Y-%m-%d").ok())
            .flatten()
            .ok_or_else(|| D::Error::custom("not a K8 day"))
    }
}
