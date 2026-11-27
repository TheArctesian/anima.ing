//! Verify a stopped builder's sealed export outside admission and receipt locks.
use super::{
    super::{Error, Result, command},
    artifact, lease, store,
};
use serde_json::{Value, json};
use std::{fs::File, time::Duration};
use tokio::time::{Instant, sleep};
use uuid::Uuid;

pub(super) struct Pending {
    id: Uuid,
    claim: Value,
    verifier: File,
}

impl Pending {
    /// The caller owns the attempt and has proved that the worker stopped.
    pub(super) async fn seal(id: Uuid, claim: Value) -> Result<Self> {
        let verifier = store::lock_verification(id)?;
        let work = store::base(id).join("work");
        command::run(
            "btrfs",
            &[
                "property",
                "set",
                "-ts",
                work.to_str().ok_or(Error::Refused("Invalid build path."))?,
                "ro",
                "true",
            ],
        )
        .await?;
        Ok(Self {
            id,
            claim,
            verifier,
        })
    }

    pub(super) async fn complete(self) -> Result<Value> {
        let Self {
            id,
            claim,
            verifier,
        } = self;
        let path = store::base(id).join("work/image.oci");
        // Keep the verification lock in the blocking task even if its caller is cancelled.
        let (_verifier, verified) =
            tokio::task::spawn_blocking(move || (verifier, artifact::verify(&path, &claim)))
                .await
                .map_err(|_| Error::Refused("Artifact verification did not complete."))?;
        let _receipt = lock_receipt(id).await?;
        let mut attempt = store::required(id)?;
        if !matches!(
            attempt.phase.as_str(),
            "preparing" | "launching" | "running" | "unknown"
        ) {
            return Ok(serde_json::to_value(attempt)?);
        }
        if lease::expired_attempt(&attempt) {
            return Err(Error::Refused(
                "The build lease expired during artifact verification.",
            ));
        }
        attempt.result = Some(receipt(&attempt, &verified?)?);
        attempt.phase = "verified".into();
        store::save(&attempt)?;
        Ok(serde_json::to_value(attempt)?)
    }
}

async fn lock_receipt(id: Uuid) -> Result<File> {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        match store::lock(id) {
            Err(Error::BuildAttemptBusy) if Instant::now() < deadline => {
                sleep(Duration::from_millis(25)).await;
            }
            result => return result,
        }
    }
}

fn receipt(attempt: &store::Attempt, verified: &artifact::Verified) -> Result<Value> {
    let source = attempt
        .source_digest
        .as_ref()
        .ok_or(Error::Refused("The build has no captured source digest."))?;
    let toolchain = attempt
        .toolchain
        .as_ref()
        .ok_or(Error::Refused("The build has no pinned toolchain receipt."))?;
    Ok(json!({
        "schema":1, "attempt":attempt.id, "context":attempt.context,
        "workspace":attempt.workspace, "generation":attempt.generation,
        "sourceDigest":source, "source":attempt.source, "strategy":attempt.strategy, "toolchain":toolchain,
        "manifestDigest":verified.manifest, "archiveDigest":verified.digest,
        "bytes":verified.bytes, "hostArchitecture":std::env::consts::ARCH,
        "integrityVerified":true, "deployed":false,
        "provenance":verified.provenance
    }))
}
