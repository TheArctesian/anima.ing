mod delivery;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{PgConnection, PgPool};
use uuid::Uuid;

pub(in crate::fleet) use delivery::{poll, result};

use super::{
    Actor, Error, Owner,
    admission::KEY,
    audit,
    control::{self, Action, Answer, StopTarget},
    store,
};

const CONTENT_LIMIT: usize = 64 * 1024;

#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub(in crate::fleet) enum Payload {
    Prompt {
        prompt: String,
    },
    Answer {
        process: String,
        revision: i64,
        request: String,
        answer: Answer,
    },
    Stop {
        target: StopTarget,
    },
}

impl Payload {
    const fn kind(&self) -> &'static str {
        match self {
            Self::Prompt { .. } => "prompt",
            Self::Answer { .. } => "answer",
            Self::Stop { .. } => "stop",
        }
    }

    fn validate(&self) -> Result<(), Error> {
        let valid = match self {
            Self::Prompt { prompt } => valid_prompt(prompt),
            Self::Answer { .. } => serde_json::to_vec(self)?.len() <= CONTENT_LIMIT,
            Self::Stop { .. } => true,
        };
        valid.then_some(()).ok_or(Error::Refused("invalid_answer"))
    }
}

pub(super) fn valid_prompt(prompt: &str) -> bool {
    !prompt.trim().is_empty() && prompt.len() <= CONTENT_LIMIT && !prompt.contains('\0')
}

#[derive(Serialize, sqlx::FromRow)]
pub(in crate::fleet) struct CommandReceipt {
    #[sqlx(rename = "command_id")]
    pub command: String,
    pub state: String,
    pub cause: Option<String>,
}

pub(in crate::fleet) async fn admit(
    (db, actor): (&PgPool, Actor),
    account: Uuid,
    (command, thread): (&str, Uuid),
    payload: Payload,
) -> Result<CommandReceipt, Error> {
    if command.is_empty() || command.len() > 256 || command.chars().any(char::is_control) {
        return Err(Error::malformed());
    }
    let mut tx = db.begin().await?;
    store::lock_id(&mut tx, account, command).await?;
    let receipt = if let Some(prior) = retry(&mut tx, account, (command, thread), &payload).await? {
        prior
    } else {
        let device = control::thread_device(&mut *tx, thread).await?;
        store::lock_device(&mut tx, device).await?;
        let at = store::now(&mut tx).await?;
        let owner = Owner { account, device };
        let receipt = admit_new(&mut tx, (owner, at), (command, thread), &payload).await?;
        audit::write(&mut tx, (owner, at), actor, (crate::fleet::events::AuditAction::Create,
            serde_json::json!({"thread": thread, "command": command, "action": payload.kind(), "outcome": "waiting"}))).await?;
        receipt
    };
    tx.commit().await?;
    Ok(receipt)
}

async fn retry(
    conn: &mut PgConnection,
    account: Uuid,
    (command, thread): (&str, Uuid),
    payload: &Payload,
) -> Result<Option<CommandReceipt>, Error> {
    let row: Option<(Uuid, Vec<u8>, i16)> = sqlx::query_as(
        "SELECT device_id, fingerprint, key_version FROM agent_threads.commands
         WHERE account_id = $1 AND command_id = $2",
    )
    .bind(account)
    .bind(command)
    .fetch_optional(&mut *conn)
    .await?;
    let Some((device, fingerprint, key)) = row else {
        return Ok(None);
    };
    let digest = store::fingerprint(conn, key, "command", &(thread, payload)).await?;
    if fingerprint != digest {
        return Err(Error::Refused("COMMAND_CONFLICT"));
    }
    store::lock_device(conn, device).await?;
    let at = store::now(conn).await?;
    store::expire_waiting(conn, device, at).await?;
    receipt(conn, account, command).await.map(Some)
}

async fn admit_new(
    conn: &mut PgConnection,
    (owner, at): (Owner, DateTime<Utc>),
    (command, thread): (&str, Uuid),
    payload: &Payload,
) -> Result<CommandReceipt, Error> {
    payload.validate()?;
    let row = control::bound(conn, owner, thread, at)
        .await?
        .ok_or(Error::Refused("thread_unknown"))?;
    check_offer(conn, (owner, at), row.id, payload).await?;
    store::expire_waiting(conn, owner.device, at).await?;
    if matches!(payload, Payload::Stop { .. }) {
        delivery::cancel_prompts(conn, owner, row.id).await?;
    }
    store::capacity(
        conn,
        owner.device,
        matches!(payload, Payload::Prompt { .. }),
    )
    .await?;
    store::reserve_record(conn, KEY).await?;
    let fingerprint = store::fingerprint(conn, KEY, "command", &(thread, payload)).await?;
    insert(conn, (owner, row.id, command, at), payload, fingerprint).await?;
    receipt(conn, owner.account, command).await
}

async fn check_offer(
    conn: &mut PgConnection,
    (owner, at): (Owner, DateTime<Utc>),
    launch: Uuid,
    payload: &Payload,
) -> Result<(), Error> {
    let offer = control::live(conn, owner, launch, at).await?;
    if !offer.is_some_and(|offer| offered(&offer, payload)) {
        return Err(Error::Refused(match payload {
            Payload::Prompt { .. } => "thread_unknown",
            Payload::Answer { .. } => "invalid_answer",
            Payload::Stop { .. } => "target_stale",
        }));
    }
    if matches!(payload, Payload::Prompt { .. }) {
        if store::blocked(conn, owner.device, at).await? {
            return Err(Error::Refused("WORK_OFF"));
        }
        if !delivery::prompt_on(conn, owner, at).await? {
            return Err(Error::Refused("threads_off"));
        }
    }
    Ok(())
}

fn offered(offer: &control::Offer, payload: &Payload) -> bool {
    match payload {
        Payload::Prompt { .. } => offer.actions.contains(&Action::Prompt),
        Payload::Answer {
            process,
            revision,
            request,
            answer,
        } => offer.accepts_answer((process, *revision, request), answer),
        Payload::Stop { target } => {
            offer.actions.contains(&Action::Stop) && offer.stop_targets.contains(target)
        }
    }
}

pub(super) async fn insert_first(
    conn: &mut PgConnection,
    (owner, launch, first): (Owner, Uuid, Uuid),
    prompt: &str,
    at: DateTime<Utc>,
) -> Result<(), Error> {
    let payload = Payload::Prompt {
        prompt: prompt.to_owned(),
    };
    let fingerprint = store::fingerprint(conn, KEY, "first-prompt", &(launch, &payload)).await?;
    insert(
        conn,
        (owner, launch, &first.to_string(), at),
        &payload,
        fingerprint,
    )
    .await
}

async fn insert(
    conn: &mut PgConnection,
    (owner, launch, command, at): (Owner, Uuid, &str, DateTime<Utc>),
    payload: &Payload,
    fingerprint: Vec<u8>,
) -> Result<(), Error> {
    sqlx::query(
        "INSERT INTO agent_threads.commands (
             account_id, command_id, device_id, launch_id, kind, payload, fingerprint,
             key_version, state, created_at, deadline
         ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,'waiting',$9,$10)",
    )
    .bind(owner.account)
    .bind(command)
    .bind(owner.device)
    .bind(launch)
    .bind(payload.kind())
    .bind(serde_json::to_value(payload)?)
    .bind(fingerprint)
    .bind(KEY)
    .bind(at)
    .bind(at + store::TEN_MINUTES)
    .execute(conn)
    .await?;
    Ok(())
}

pub(in crate::fleet) async fn command_receipt(
    db: &PgPool,
    account: Uuid,
    id: &str,
) -> Result<CommandReceipt, Error> {
    let mut tx = db.begin().await?;
    let device: Option<Uuid> = sqlx::query_scalar(
        "SELECT device_id FROM agent_threads.commands WHERE account_id = $1 AND command_id = $2",
    )
    .bind(account)
    .bind(id)
    .fetch_optional(&mut *tx)
    .await?;
    let device = device.ok_or(Error::Refused("COMMAND_UNKNOWN"))?;
    store::lock_device(&mut tx, device).await?;
    let at = store::now(&mut tx).await?;
    store::expire_waiting(&mut tx, device, at).await?;
    let receipt = receipt(&mut tx, account, id).await?;
    tx.commit().await?;
    Ok(receipt)
}

async fn receipt(
    conn: &mut PgConnection,
    account: Uuid,
    id: &str,
) -> Result<CommandReceipt, Error> {
    sqlx::query_as(
        "SELECT command_id, state, cause FROM agent_threads.commands
         WHERE account_id = $1 AND command_id = $2",
    )
    .bind(account)
    .bind(id)
    .fetch_optional(conn)
    .await?
    .ok_or(Error::Refused("COMMAND_UNKNOWN"))
}
