//! Wire identity and receipt for a retained source artifact.
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const MAX_ARCHIVE_BYTES: u64 = 8 * 1024 * 1024 * 1024;

pub const ROUTE: &str = "/work/v1/artifact";
pub const RECEIPT: &str = "anima-artifact-receipt";
pub const JOB: &str = "anima-artifact-job";
pub const ATTEMPT: &str = "anima-artifact-attempt";
pub const BYTES: &str = "anima-artifact-bytes";
pub const ARCHIVE: &str = "anima-artifact-archive";
pub const MANIFEST: &str = "anima-artifact-manifest";

/// A private OCI archive, bound to one source-build attempt.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Offer {
    pub job: Uuid,
    pub attempt: u32,
    pub bytes: u64,
    pub archive_digest: String,
    pub manifest_digest: String,
}

impl Offer {
    #[must_use]
    pub fn valid(&self) -> bool {
        !self.job.is_nil()
            && self.attempt > 0
            && (1..=MAX_ARCHIVE_BYTES).contains(&self.bytes)
            && digest(&self.archive_digest)
            && digest(&self.manifest_digest)
    }
}

fn digest(value: &str) -> bool {
    value.strip_prefix("sha256:").is_some_and(|hex| {
        hex.len() == 64
            && hex
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    })
}

/// Durable private retention only; this receipt grants no deployment or publication.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Receipt {
    pub version: u8,
    pub id: Uuid,
    pub device: Uuid,
    pub artifact: Offer,
}

impl Receipt {
    #[must_use]
    pub fn valid(&self) -> bool {
        self.version == 1 && !self.id.is_nil() && !self.device.is_nil() && self.artifact.valid()
    }
}
