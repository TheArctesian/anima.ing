use serde::{Deserialize, Deserializer, Serialize, Serializer, de::DeserializeOwned};
use serde_json::Value;

/// A failed probe never carries an old or partial successful result.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Probe<T> {
    Read(T),
    Unavailable(State),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum State {
    Missing,
    Unsupported,
    TimedOut,
    Exited,
    InvalidOutput,
    TooLarge,
    #[serde(other)]
    Unknown,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entries<T> {
    pub entries: Vec<T>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Version {
    pub version: String,
}

impl<T: Serialize> Serialize for Probe<T> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        #[derive(Serialize)]
        struct Read<'a, T> {
            state: &'static str,
            #[serde(flatten)]
            found: &'a T,
        }
        #[derive(Serialize)]
        struct Failed {
            state: State,
        }
        match self {
            Self::Read(found) => Read {
                state: "read",
                found,
            }
            .serialize(serializer),
            Self::Unavailable(state) => Failed { state: *state }.serialize(serializer),
        }
    }
}

impl<'de, T: DeserializeOwned> Deserialize<'de> for Probe<T> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        use serde::de::Error as _;
        let mut value = Value::deserialize(deserializer)?;
        let object = value
            .as_object_mut()
            .ok_or_else(|| D::Error::custom("probe object required"))?;
        let state = object
            .remove("state")
            .ok_or_else(|| D::Error::custom("probe state required"))?;
        if state == "read" {
            if object.contains_key("entries") == object.contains_key("version") {
                return Err(D::Error::custom("one probe result required"));
            }
            serde_json::from_value(value)
                .map(Self::Read)
                .map_err(D::Error::custom)
        } else {
            let state: State = serde_json::from_value(state).map_err(D::Error::custom)?;
            if state != State::Unknown
                && (object.contains_key("entries") || object.contains_key("version"))
            {
                return Err(D::Error::custom("failed probe carries a result"));
            }
            Ok(Self::Unavailable(state))
        }
    }
}

impl<T> Probe<Entries<T>> {
    pub(super) fn valid(&self, limit: usize, entry: impl Fn(&T) -> (&str, bool)) -> bool {
        let Self::Read(found) = self else { return true };
        found.entries.len() <= limit
            && found.entries.iter().all(|value| entry(value).1)
            && found
                .entries
                .windows(2)
                .all(|pair| entry(&pair[0]).0 < entry(&pair[1]).0)
    }
}
