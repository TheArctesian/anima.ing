//! Queue a managed Compose apply and release its exact encrypted inputs once to its holder.
use chrono::{DateTime, TimeDelta, Utc};
use daemon_core::wire::{
    ApplicationApply, COMPOSE_FILE, ENVIRONMENT_FILE, Limits, SecretRequest, WorkKind, WorkOs,
    WorkType,
};
use sha2::{Digest as _, Sha256};
use sqlx::PgConnection;
use uuid::Uuid;

use crate::{
    clock::Clock,
    fleet::{
        credentials::{Sealed, Secret, Vault},
        events::{ActorKind, ClientAddr, TokenKind},
    },
    ids::{DeviceId, JobId},
};

use super::{Asker, Error, Job, NewJob, Queue, State, ended, hosting, queued, store, trail};
use store::application_files;

const APPLY_SECONDS: u64 = 1800;

/// Plaintext from the operator. The caller never keeps a copy in the job or an audit record.
pub struct ApplicationFiles {
    pub compose: Secret,
    pub environment: Option<Secret>,
}

/// The plaintext for one successful release, with the deadline of the held attempt.
pub struct ReleasedApplicationFile {
    pub value: Secret,
    pub expires_at: DateTime<Utc>,
}

/// Queue a signed-in operator's files for the named application and device.
/// The application binding, queue entry and encrypted files commit together.
///
/// # Errors
/// Wrong actor, impermissible files, conflicting target, or storage failure.
pub async fn enqueue_application_apply(
    (queue, vault): (Queue<'_>, &Vault),
    (application, device, pull, keep): (Uuid, DeviceId, bool, bool),
    files: ApplicationFiles,
    (asker, now): (Asker, DateTime<Utc>),
) -> Result<Job, Error> {
    let actor = &asker.0.actor;
    if actor.kind() != ActorKind::User
        || !matches!(actor.token(), Some(TokenKind::Login | TokenKind::Browser))
    {
        return Err(Error::Invalid(
            "a Compose file requires an operator session",
        ));
    }
    if application.is_nil() {
        return Err(Error::Invalid("the application must have an identifier"));
    }
    let files = checked(files).await?;
    let parameters = ApplicationApply {
        application,
        file_sha256: hash(&files.compose),
        environment_sha256: files.environment.as_ref().map(hash),
        pull,
        keep: u8::from(keep),
    };
    let job = queued(wanted(device, &parameters)?, now);
    let sealed = seal(vault, job.id, device, &files)?;
    drop(files);
    let mut tx = queue.db.begin().await?;
    let outcome = async {
        hosting::admit(&mut tx, &job, &asker.0, now).await?;
        if !application_files::bind(&mut tx, application, device).await? {
            return Err(Error::Invalid("the application belongs to another machine"));
        }
        store::insert(&mut tx, &job, None).await?;
        for (purpose, sealed) in sealed {
            application_files::insert(&mut tx, job.id, purpose, &sealed).await?;
        }
        trail::queued(&mut tx, &job, asker, now).await
    }
    .await;
    ended(tx, outcome).await.map(|()| job)
}

async fn checked(files: ApplicationFiles) -> Result<ApplicationFiles, Error> {
    let compose_len = files.compose.expose().len();
    let environment_len = files
        .environment
        .as_ref()
        .map_or(0, |file| file.expose().len());
    if compose_len > compose_rules::LARGEST_FILE
        || environment_len > compose_rules::LARGEST_ENVIRONMENT
    {
        return Err(Error::Invalid(
            "the application files exceed their size limit",
        ));
    }
    tokio::task::spawn_blocking(move || {
        let compose = utf8(&files.compose)?;
        let environment = files
            .environment
            .as_ref()
            .map(utf8)
            .transpose()?
            .unwrap_or_default();
        compose_rules::check_portable(compose, environment)
            .map_err(|_| Error::Invalid("the application files break a Compose rule"))?;
        Ok(files)
    })
    .await
    .map_err(|_| Error::Unreadable)?
}

fn utf8(value: &Secret) -> Result<&str, Error> {
    std::str::from_utf8(value.expose())
        .map_err(|_| Error::Invalid("an application file must be UTF-8"))
}

fn hash(value: &Secret) -> String {
    use std::fmt::Write as _;
    let mut text = String::with_capacity(64);
    for byte in Sha256::digest(value.expose()) {
        let _ = write!(text, "{byte:02x}");
    }
    text
}

fn wanted(device: DeviceId, parameters: &ApplicationApply) -> Result<NewJob, Error> {
    let mut secrets = vec![COMPOSE_FILE.to_owned()];
    if parameters.environment_sha256.is_some() {
        secrets.push(ENVIRONMENT_FILE.to_owned());
    }
    Ok(NewJob {
        work_type: WorkType::ApplicationApply,
        kind: WorkKind::Host,
        os: Some(WorkOs::Linux),
        architecture: None,
        project: None,
        repository: None,
        parameters: serde_json::to_value(parameters).map_err(|_| Error::Unreadable)?,
        secrets,
        limits: Limits {
            seconds: APPLY_SECONDS,
            idle_seconds: 1,
            memory_bytes: 1,
            disk_bytes: 1,
            orphan_seconds: 1,
        },
        machine: Some(device),
    })
}

fn seal(
    vault: &Vault,
    job: JobId,
    device: DeviceId,
    files: &ApplicationFiles,
) -> Result<Vec<(&'static str, Vec<u8>)>, Error> {
    let required = std::iter::once((COMPOSE_FILE, &files.compose));
    let optional = files
        .environment
        .as_ref()
        .map(|file| (ENVIRONMENT_FILE, file));
    required
        .chain(optional)
        .map(|(purpose, file)| {
            vault
                .seal(file, &context(job, device, purpose))
                .map(|sealed| (purpose, sealed.to_bytes()))
                .map_err(|_| Error::Unreadable)
        })
        .collect()
}

fn context(job: JobId, device: DeviceId, purpose: &str) -> Vec<u8> {
    format!("jobs.application_files:{job}:{device}:{purpose}").into_bytes()
}

/// Consume one declared input of the authenticated daemon's current apply.
///
/// # Errors
/// A database, audit or sealing failure prevents releasing the file.
pub async fn take_application_file(
    (queue, vault, clock): (Queue<'_>, &Vault, &dyn Clock),
    (device, client): (DeviceId, ClientAddr),
    request: &SecretRequest,
) -> Result<Option<ReleasedApplicationFile>, Error> {
    if !matches!(request.purpose.as_str(), COMPOSE_FILE | ENVIRONMENT_FILE) {
        return Ok(None);
    }
    let mut tx = queue.db.begin().await?;
    let outcome = take(&mut tx, (vault, clock), (device, client), request).await;
    if let Ok(Some(file)) = &outcome
        && clock.now() >= file.expires_at
    {
        tx.rollback().await?;
        return Ok(None);
    }
    ended(tx, outcome).await
}

async fn take(
    conn: &mut PgConnection,
    (vault, clock): (&Vault, &dyn Clock),
    (device, client): (DeviceId, ClientAddr),
    request: &SecretRequest,
) -> Result<Option<ReleasedApplicationFile>, Error> {
    let id = JobId::from(request.id);
    let Some(job) = store::job_locked(conn, id).await? else {
        return Ok(None);
    };
    if job.asked.work_type != WorkType::ApplicationApply
        || job.asked.machine != Some(device)
        || job.holder != Some(device)
        || job.attempt != request.attempt
        || job.stop.is_some()
        || !matches!(job.state, State::Preparing | State::Running)
        || !job.asked.secrets.contains(&request.purpose)
    {
        return Ok(None);
    }
    let parameters: ApplicationApply =
        serde_json::from_value(job.asked.parameters.clone()).map_err(|_| Error::Unreadable)?;
    if !application_files::bound(conn, parameters.application, device).await? {
        return Ok(None);
    }
    let Some(expires_at) = expires(&job, clock.now()) else {
        return Ok(None);
    };
    let Some(sealed) = application_files::take(conn, id, &request.purpose).await? else {
        return Ok(None);
    };
    let sealed = Sealed::from_bytes(&sealed).map_err(|_| Error::Unreadable)?;
    let value = vault
        .open(&sealed, &context(id, device, &request.purpose))
        .map_err(|_| Error::Unreadable)?;
    let now = clock.now();
    trail::secret_released(conn, &job, (&request.purpose, device), (client, now)).await?;
    Ok(Some(ReleasedApplicationFile { value, expires_at }))
}

fn expires(job: &Job, now: DateTime<Utc>) -> Option<DateTime<Utc>> {
    let limit = TimeDelta::seconds(i64::try_from(job.asked.limits.seconds).ok()?);
    let deadline = job.claimed_at?.checked_add_signed(limit)?;
    let expires = job.lease_ends_at?.min(deadline);
    (now < expires).then_some(expires)
}
