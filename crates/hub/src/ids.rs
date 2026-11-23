//! One UUID newtype per entity, so a `DeviceId` never passes for a `SiteId`; domains generate
//! ids at their I/O edge.

use std::{fmt, str::FromStr};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, thiserror::Error)]
#[error("not a valid id")]
pub struct ParseIdError(#[from] uuid::Error);

macro_rules! entity_id {
    ($($(#[$doc:meta])* $name:ident),+ $(,)?) => {$(
        $(#[$doc])*
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, sqlx::Type)]
        #[sqlx(transparent)]
        pub struct $name(Uuid);

        impl FromStr for $name {
            type Err = ParseIdError;

            fn from_str(text: &str) -> Result<Self, Self::Err> {
                Ok(Self(Uuid::parse_str(text)?))
            }
        }

        impl From<Uuid> for $name {
            fn from(uuid: Uuid) -> Self {
                Self(uuid)
            }
        }

        impl From<$name> for Uuid {
            fn from(id: $name) -> Self {
                id.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                self.0.hyphenated().fmt(f)
            }
        }
    )+};
}

entity_id! {
    OperatorId,
    SiteId,
    DeviceId,
    CheckId,
    WindowId,
    SessionId,
    EventId,
    WebInterfaceId,
    MetricSeriesId,
    ApiKeyId,
    ChannelId,
    DeliveryId,
    TokenId,
    ReleaseId,
    BuildJobId,
    BuilderId,
    /// A job of the work route: what the hub gives to the daemon of a machine (K10).
    JobId,
    SourceAccountId,
    SourceSecretId,
    SyncRunId,
    ProjectId,
    /// A group of projects: the operator's own label (PRO-059).
    ProjectGroupId,
    RepositoryId,
    ResourceId,
    /// A link between two records (RLK-001).
    RecordLinkId,
    /// A place, as every extension names it (RLK-004).
    PlaceId,
    /// A person, as every extension names them (RLK-004).
    PersonId,
    /// A file of one record (FIL-001).
    AttachmentId,
    /// An archive: the unit of deletion of one import (FIL-004).
    ArchiveId,
    /// One extraction proposal (VIS-001).
    ProposalId,
    /// An operational key: a credential a server of the operator needs (R-147).
    OperationalKeyId,
    /// One version of an operational key; its id is in the context of its sealed value.
    OperationalKeyVersionId,
    /// One location sample of the operator's phone (K-E4 Section 11).
    SampleId,
    /// An ingest token of the OwnTracks path (K-E4 Section 11).
    IngestTokenId,
    /// A reminder rule (K-E6 Section 2).
    ReminderRuleId,
    /// One firing of a reminder rule (K-E6 Section 3).
    FiringId,
    /// A record that a client sealed (K-E5 Section 3).
    SealedRecordId,
    /// An offer of the vault key to a new client (K-E5 Section 3).
    KeyOfferId,
    /// An application on a machine, as the applications program names it (HOST-008).
    ApplicationId,
    /// A public name of an application on the hub (HOST-046, task H21).
    RouteId,
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEXT: &str = "0192f5a0-7c3e-7a52-9d1b-3f4e5a6b7c8d";

    #[test]
    fn id_text_round_trips_through_from_str_and_display() {
        let id: DeviceId = TEXT.parse().unwrap();
        assert_eq!(id.to_string(), TEXT);
    }

    #[test]
    fn id_converts_to_and_from_a_generated_uuid() {
        let uuid = Uuid::now_v7();
        let id = EventId::from(uuid);
        assert_eq!(Uuid::from(id), uuid);
        assert_eq!(id.to_string(), uuid.hyphenated().to_string());
    }

    #[test]
    fn id_rejects_text_that_is_not_a_uuid() {
        assert!("not-an-id".parse::<SiteId>().is_err());
    }

    #[test]
    fn id_serialises_as_the_uuid_string() {
        let id: CheckId = TEXT.parse().unwrap();
        assert_eq!(serde_json::to_string(&id).unwrap(), format!("\"{TEXT}\""));
    }
}
