use super::{Error, Phase, Result, authorized};
use crate::linux::builds::export;
use daemon_core::source_jobs::{Action, Request};
use std::time::Duration;
use tokio::{io::AsyncWriteExt, net::unix::OwnedWriteHalf};

pub(super) async fn send(request: Request, mut output: OwnedWriteHalf) -> Result<()> {
    let Action::Export {
        execution,
        archive_digest,
        manifest_digest,
    } = request.action
    else {
        return Err(Error::Refused("Choose an artifact export request."));
    };
    let record = authorized(execution, request.device, &request.origin)?;
    if record.phase != Phase::Ready
        || !record.result.as_ref().is_some_and(|result| {
            result["integrityVerified"] == true
                && result["archiveDigest"] == archive_digest
                && result["manifestDigest"] == manifest_digest
        })
    {
        return Err(Error::Refused(
            "This source execution has no ready artifact.",
        ));
    }
    let artifact = export::open(record.build, &archive_digest, &manifest_digest).await?;
    let mut header = serde_json::to_vec(&serde_json::json!({"ok":true,"value":{
        "bytes":artifact.bytes,"archiveDigest":archive_digest,"manifestDigest":manifest_digest
    }}))?;
    header.push(b'\n');
    tokio::time::timeout(Duration::from_secs(5), output.write_all(&header))
        .await
        .map_err(|_| Error::Refused("Artifact header delivery timed out."))??;
    artifact.write_to(&mut output).await
}
