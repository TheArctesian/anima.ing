use super::{
    Error, Grant, Lease, Phase, Record, Result, Utc, Uuid, Value, WorkType, current, store,
    work_policy,
};

pub(super) fn register(
    execution: Uuid,
    enrollment: (Uuid, String),
    item: &Value,
    coordinated: bool,
) -> Result<Value> {
    let (device, origin) = enrollment;
    validate_identity(execution, device, &origin)?;
    let mut job = work_policy::check_item(item)?;
    if job.work_type != WorkType::Build
        || job.parameters["recipe"] != "source_image_v1"
        || job.parameters["target"] != format!("linux-{}", std::env::consts::ARCH)
    {
        return Err(Error::Refused(
            "This node cannot register the requested source recipe or architecture.",
        ));
    }
    if !(job.secrets.is_empty() || coordinated && daemon_core::source_jobs::private_checkout(&job))
    {
        return Err(Error::Refused(
            "This source job has unsupported checkout credentials.",
        ));
    }
    if let Some(existing) = store::read(execution)? {
        // Registration retries are not lease renewals.
        job.lease = existing.job.lease;
        if existing.device != device
            || existing.origin != origin
            || existing.job != job
            || existing.coordinated != coordinated
        {
            return Err(Error::Refused(
                "This execution identity belongs to another source-job request.",
            ));
        }
        return Ok(serde_json::to_value(current(execution)?)?);
    }
    store::accept(device, &origin, &job)?;
    let lease = Lease::new(Grant {
        job: job.id,
        attempt: job.attempt,
        until: job.lease.until,
    })?;
    let record = Record {
        schema: 1,
        execution,
        device,
        origin,
        job,
        lease,
        registered_at: Utc::now(),
        coordinated,
        phase: Phase::Registered,
        workspace: Uuid::now_v7(),
        context: Uuid::now_v7(),
        build: Uuid::now_v7(),
        result: None,
        failure: None,
        release_receipt: None,
    };
    store::save(&record)?;
    Ok(serde_json::to_value(record)?)
}

fn validate_identity(execution: Uuid, device: Uuid, origin: &str) -> Result<()> {
    if execution.is_nil()
        || device.is_nil()
        || origin.len() > 512
        || !origin.strip_prefix("https://").is_some_and(|host| {
            !host.is_empty()
                && host
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b".-:[]".contains(&b))
        })
    {
        return Err(Error::Refused(
            "Choose valid source-job execution and enrollment identities.",
        ));
    }
    Ok(())
}
