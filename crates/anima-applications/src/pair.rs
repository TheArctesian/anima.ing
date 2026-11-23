//! The one hub that the settings are made for (contract K10 Section 6.4.2).

use crate::{
    forms::{is_origin, is_uuid},
    strict::{self, Members},
};

/// The origin of a hub and the device identifier of the machine at that hub.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pair {
    origin: String,
    device: String,
}

impl Pair {
    /// The pair of a request; `None` when a part does not have its form. The origin is the
    /// scheme, the host in lower case and the port, always written; the device identifier is
    /// a UUID of 36 lower-case characters.
    #[must_use]
    pub fn of(origin: &str, device: &str) -> Option<Self> {
        (is_origin(origin) && is_uuid(device)).then(|| Self {
            origin: origin.to_owned(),
            device: device.to_owned(),
        })
    }
}

/// The record of the first pair, in a file of the applications program.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PairRecord {
    /// The time of "Manage applications" that the record was made for; a new time makes the record not valid.
    pub set_on_at: u64,
    pub first: Pair,
    /// True when a request had a different pair. Each container item is then refused until
    /// the administrator sets "Manage applications" on again.
    pub different_seen: bool,
}

/// The file of the record of the pair, as the caller found it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PairFile<'a> {
    Missing,
    /// The file cannot be read or parsed: each item is refused, as no record would accept a different hub.
    Unreadable,
    Read(&'a PairRecord),
}

/// What the pair of one request means.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PairDecision {
    Recorded,
    /// No valid record exists: the pair is the first, and the program writes this record
    /// before it does the work.
    First(PairRecord),
    /// The pair is different, now or at an earlier request: the item is refused
    /// (`applications_off`). The program writes the record when it has one here.
    Different(Option<PairRecord>),
}

#[must_use]
pub fn decided(file: PairFile<'_>, set_on_at: u64, sent: &Pair) -> PairDecision {
    let record = match file {
        PairFile::Missing => None,
        PairFile::Unreadable => return PairDecision::Different(None),
        PairFile::Read(record) => Some(record),
    };
    match record.filter(|record| record.set_on_at == set_on_at) {
        None => PairDecision::First(PairRecord {
            set_on_at,
            first: sent.clone(),
            different_seen: false,
        }),
        Some(record) if record.different_seen => PairDecision::Different(None),
        Some(record) if record.first == *sent => PairDecision::Recorded,
        Some(record) => PairDecision::Different(Some(PairRecord {
            different_seen: true,
            ..record.clone()
        })),
    }
}

impl PairRecord {
    #[must_use]
    pub fn written(&self) -> String {
        // The origin and the device have forms with no character that JSON escapes.
        format!(
            "{{\"setOnAt\":{},\"origin\":\"{}\",\"device\":\"{}\",\"differentSeen\":{}}}\n",
            self.set_on_at, self.first.origin, self.first.device, self.different_seen
        )
    }

    /// Reads the text of the file of the record; `None` for text that [`Self::written`] does not write.
    #[must_use]
    pub fn read(text: &str) -> Option<Self> {
        let mut members = Members::of(strict::read(text)?)?;
        let record = Self {
            set_on_at: members.whole("setOnAt")?,
            first: Pair::of(&members.text("origin")?, &members.text("device")?)?,
            different_seen: members.truth("differentSeen")?,
        };
        members.is_empty().then_some(record)
    }
}
