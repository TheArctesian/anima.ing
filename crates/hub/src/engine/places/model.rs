//! A place and the words around it (K-E4 Section 8; PLC-001 to PLC-006).

use chrono::{DateTime, Utc};

use crate::ids::{PlaceId, RecordLinkId, SiteId};

/// The hub keeps this closed list; the client chooses the words.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Category {
    Home,
    Work,
    Site,
    Shop,
    Leisure,
    Travel,
    Storage,
    Other,
}

impl Category {
    pub const ALL: [Self; 8] = [
        Self::Home,
        Self::Work,
        Self::Site,
        Self::Shop,
        Self::Leisure,
        Self::Travel,
        Self::Storage,
        Self::Other,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Home => "home",
            Self::Work => "work",
            Self::Site => "site",
            Self::Shop => "shop",
            Self::Leisure => "leisure",
            Self::Travel => "travel",
            Self::Storage => "storage",
            Self::Other => "other",
        }
    }

    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|category| category.as_str() == text)
    }
}

/// A child place has none (PLC-002).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Centre {
    pub latitude: f64,
    pub longitude: f64,
    pub radius_m: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Place {
    pub id: PlaceId,
    pub parent: Option<PlaceId>,
    /// The name the operator gave; a geocoder never changes it (PLC-003).
    pub name: String,
    pub centre: Option<Centre>,
    pub category: Category,
    pub icon: String,
    pub aliases: Vec<String>,
    pub geocoder_name: Option<String>,
    pub merged_into: Option<PlaceId>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Draft {
    pub name: String,
    pub parent: Option<PlaceId>,
    pub centre: Option<Centre>,
    pub category: Category,
    pub icon: String,
    pub aliases: Vec<String>,
    pub geocoder_name: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Entry {
    pub name: String,
    pub parent: Option<PlaceId>,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
    pub radius_m: Option<i32>,
    pub category: String,
    pub icon: String,
    pub aliases: Vec<String>,
    pub geocoder_name: Option<String>,
}

/// A field left `None` keeps its value.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Patch {
    pub name: Option<String>,
    /// `Some(None)` makes the place a top-level place; `Some(Some(id))` puts it inside `id`.
    pub parent: Option<Option<PlaceId>>,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
    pub radius_m: Option<i32>,
    pub category: Option<String>,
    pub icon: Option<String>,
    pub aliases: Option<Vec<String>>,
    /// `Some(None)` removes the geocoder name.
    pub geocoder_name: Option<Option<String>>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Split {
    pub new: Entry,
    pub children: Vec<PlaceId>,
    pub sites: Vec<SiteId>,
    pub links: Vec<RecordLinkId>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Fault {
    pub field: &'static str,
    pub message: &'static str,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct References {
    pub links: u64,
    pub sites: u64,
    pub children: u64,
    /// One count for each later domain that holds a `PlaceId`.
    pub held: Vec<(&'static str, u64)>,
}

impl References {
    #[must_use]
    pub fn total(&self) -> u64 {
        self.links + self.sites + self.children + self.held.iter().map(|(_, n)| n).sum::<u64>()
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Moved {
    pub links: u64,
    pub sites: u64,
    pub children: u64,
    pub held: Vec<(&'static str, u64)>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Merged {
    pub from: Place,
    pub into: Place,
    pub moved: Moved,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Divided {
    /// The place that was split, as it was.
    pub from: Place,
    pub new: Place,
    pub moved: Moved,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Removal {
    Deleted(Place),
    Replaced(Box<Merged>),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Siting {
    pub site: SiteId,
    pub before: Option<PlaceId>,
    pub after: Option<PlaceId>,
}
