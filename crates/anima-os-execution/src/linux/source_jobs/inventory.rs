use super::{Error, Phase, Result, current, store};
use serde_json::{Value, json};
use uuid::Uuid;

// The request dispatcher holds the journal lock for this complete snapshot.
pub(super) fn list(device: Uuid, origin: &str, coordinated_only: bool) -> Result<Value> {
    if device.is_nil() || origin.is_empty() || origin.len() > 512 {
        return Err(Error::Refused(
            "Choose a valid source-job enrollment identity.",
        ));
    }
    let mut ids = store::ids()?;
    ids.sort_unstable();
    let mut executions = Vec::new();
    for id in ids {
        let record =
            store::read(id)?.ok_or(Error::Refused("A source-job registration disappeared."))?;
        if record.device != device
            || record.origin != origin
            || (coordinated_only && !record.coordinated)
        {
            continue;
        }
        let record = current(id)?;
        executions.push(json!({
            "execution":record.execution, "job":record.job.id, "attempt":record.job.attempt,
            "phase":record.phase, "lease":record.lease,
            "workspace":record.workspace, "context":record.context, "build":record.build,
            "coordinated":record.coordinated, "failure":record.failure, "artifactAvailable":record.phase == Phase::Ready && record.result.is_some()
        }));
    }
    Ok(json!({"executions":executions,"truncated":false}))
}
