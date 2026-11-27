//! Live human sign-in events; neither challenges nor provider credentials are stored.
mod replies;
mod wire;
use crate::{
    api_errors::{ErrorCode, coded, pool, schema_dependency},
    clock::Clock,
    fleet::{access::Authenticated, devices, targets},
    ids::DeviceId,
};
use agent_threads::managed::provider_login::{Login, Provider};
use async_graphql::{
    Context, ID, InputObject, Json, Object, Result, Subscription,
    futures_util::{Stream, stream},
};
use chrono::{DateTime, TimeDelta, Utc};
use replies::Lease;
pub use replies::Replies;
use serde_json::{Value, json};
use sqlx::PgPool;
use std::{net::Ipv6Addr, sync::Arc, time::Duration};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, BufReader, ReadHalf, WriteHalf};
use uuid::Uuid;
use wire::{Event, claude_url, receive, send};
use zeroize::Zeroizing;

#[derive(InputObject)]
pub struct ManagedCodeProviderLoginInput {
    workspace: ID,
    generation: String,
    provider: Option<String>,
}

#[derive(Default)]
pub struct Subscription;

#[Subscription(name = "ManagedProviderSubscription")]
impl Subscription {
    #[graphql(guard = "Authenticated")]
    async fn managed_code_provider_login(
        &self,
        ctx: &Context<'_>,
        device: ID,
        input: ManagedCodeProviderLoginInput,
    ) -> Result<impl Stream<Item = Result<Json<Value>>>> {
        let ManagedCodeProviderLoginInput {
            workspace,
            generation,
            provider,
        } = input;
        let principal = super::graphql::person(ctx)?;
        let device: DeviceId = device.parse().map_err(|_| refused())?;
        let clock = schema_dependency::<Arc<dyn Clock>>(ctx)?.clone();
        let now = clock.now();
        let deadline = tokio::time::Instant::now() + Duration::from_secs(600);
        let login = Login {
            session: Uuid::now_v7(),
            principal,
            workspace: workspace.parse().map_err(|_| refused())?,
            generation: generation.parse().map_err(|_| refused())?,
            provider: match provider.as_deref().unwrap_or("codex") {
                "codex" => Provider::Codex,
                "claude" => Provider::Claude,
                _ => return Err(refused()),
            },
            valid_until: now + TimeDelta::seconds(15),
            expires_at: now + TimeDelta::minutes(10),
        };
        let pool = pool(ctx)?.clone();
        if !login.valid(now) || !authorized(&pool, device, &login, now).await {
            return Err(refused());
        }
        let node = devices::get(&pool, device).await.map_err(|_| refused())?;
        let address = node.host.parse::<Ipv6Addr>().map_err(|_| refused())?;
        let policy = schema_dependency::<targets::AddressPolicy>(ctx)?;
        let pipe = targets::provider_login(policy, address, &login)
            .await
            .map_err(|_| refused())?;
        let reply = if matches!(login.provider, Provider::Claude) {
            Some(
                schema_dependency::<Replies>(ctx)?
                    .open(device, &login, deadline)
                    .ok_or_else(refused)?,
            )
        } else {
            None
        };
        let (reading, writing) = tokio::io::split(pipe);
        let active = Active {
            pool,
            device,
            login,
            clock,
            pipe: BufReader::new(reading),
            writing,
            reply,
            submitted: false,
            deadline,
            challenge_sent: false,
        };
        Ok(stream::unfold(Some(active), next))
    }
}

struct Active {
    pool: PgPool,
    device: DeviceId,
    login: Login,
    clock: Arc<dyn Clock>,
    pipe: BufReader<ReadHalf<targets::Reached>>,
    writing: WriteHalf<targets::Reached>,
    reply: Option<Lease>,
    submitted: bool,
    deadline: tokio::time::Instant,
    challenge_sent: bool,
}

async fn authorized(pool: &PgPool, device: DeviceId, login: &Login, now: DateTime<Utc>) -> bool {
    let Ok(generation) = i64::try_from(login.generation) else {
        return false;
    };
    let lookup = sqlx::query_scalar::<_, bool>(
        "SELECT w.enabled AND w.ready AND w.generation=$3 AND w.seen_at >= $4
         AND o.principal=$5 AND NOT o.deleted FROM agent_threads.workspaces w
         JOIN agent_threads.workspace_owners o USING (device_id,workspace)
         WHERE w.device_id=$1 AND w.workspace=$2",
    )
    .bind(device)
    .bind(login.workspace)
    .bind(generation)
    .bind(now - TimeDelta::seconds(30))
    .bind(login.principal)
    .fetch_optional(pool);
    tokio::time::timeout(Duration::from_secs(2), lookup)
        .await
        .is_ok_and(|result| result.is_ok_and(|ready| ready == Some(true)))
}

async fn next(active: Option<Active>) -> Option<(Result<Json<Value>>, Option<Active>)> {
    let mut active = active?;
    let bytes = match read(&mut active).await {
        Ok(Some(bytes)) => bytes,
        Ok(None) => return Some((Ok(Json(json!({"state":"expired"}))), None)),
        Err(error) => return Some((Err(error), None)),
    };
    let event = serde_json::from_slice::<Event>(&bytes);
    match event {
        Ok(Event::Challenge {
            verification_url,
            code,
        }) if !active.challenge_sent
            && matches!(active.login.provider, Provider::Codex)
            && verification_url == "https://auth.openai.com/codex/device"
            && !code.is_empty()
            && code.len() <= 64
            && code.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-') =>
        {
            active.challenge_sent = true;
            Some((
                Ok(Json(
                    json!({"state":"challenge","verification_url":verification_url,"code":code}),
                )),
                Some(active),
            ))
        }
        Ok(Event::PasteChallenge { verification_url })
            if !active.challenge_sent
                && matches!(active.login.provider, Provider::Claude)
                && claude_url(&verification_url)
                && active.reply.as_ref().is_some_and(Lease::ready) =>
        {
            active.challenge_sent = true;
            Some((
                Ok(Json(
                    json!({"state":"paste-challenge", "verification_url":verification_url, "session":active.login.session}),
                )),
                Some(active),
            ))
        }
        Ok(Event::Succeeded)
            if active.challenge_sent
                && (matches!(active.login.provider, Provider::Codex) || active.submitted) =>
        {
            Some((confirm_success(&active).await, None))
        }
        Ok(Event::Failed) => Some((Ok(Json(json!({"state":"failed"}))), None)),
        Ok(Event::Cancelled) => Some((Ok(Json(json!({"state":"cancelled"}))), None)),
        Ok(Event::Expired) => Some((Ok(Json(json!({"state":"expired"}))), None)),
        _ => Some((Err(refused()), None)),
    }
}

async fn confirm_success(active: &Active) -> Result<Json<Value>> {
    let now = active.clock.now();
    if now >= active.login.expires_at
        || !authorized(&active.pool, active.device, &active.login, now).await
    {
        return Err(refused());
    }
    #[cfg(unix)]
    super::super::handoffs::continuation::provider_ready(
        &active.pool,
        active.device,
        &active.login,
        now,
    )
    .await
    .map_err(crate::api_errors::internal)?;
    Ok(Json(json!({"state":"succeeded"})))
}

async fn read(active: &mut Active) -> Result<Option<Zeroizing<Vec<u8>>>> {
    let mut bytes = Zeroizing::new(Vec::new());
    {
        let read = async {
            let mut bounded = (&mut active.pipe).take(4097);
            bounded.read_until(b'\n', &mut bytes).await
        };
        tokio::pin!(read);
        let mut recheck = tokio::time::interval(Duration::from_secs(1));
        loop {
            tokio::select! {
                biased;
                () = tokio::time::sleep_until(active.deadline) => return Ok(None),
                _ = recheck.tick() => {
                    let now = active.clock.now();
                    if now >= active.login.expires_at || !authorized(&active.pool, active.device, &active.login, now).await {
                        return Err(refused());
                    }
                    recheck.reset();
                }
                code = receive(&mut active.reply), if active.challenge_sent && !active.submitted && active.reply.is_some() => {
                    let Some(code) = code else { return Err(refused()); };
                    if !authorized(&active.pool, active.device, &active.login, active.clock.now()).await {
                        return Err(refused());
                    }
                    if send(&mut active.writing, &code, active.deadline).await.is_err() { return Err(refused()); }
                    active.submitted = true;
                }
                result = &mut read => {
                    if !matches!(result, Ok(1..=4096)) { return Err(refused()); }
                    break;
                }
            }
        }
    }
    if !bytes.ends_with(b"\n") {
        return Err(refused());
    }
    Ok(Some(bytes))
}

fn refused() -> async_graphql::Error {
    coded(
        "Provider sign-in is unavailable for this workspace.",
        ErrorCode::Forbidden,
    )
}

#[derive(Default)]
pub struct Mutation;

#[Object(name = "ManagedProviderMutation")]
impl Mutation {
    #[graphql(guard = "Authenticated")]
    async fn reply_managed_code_provider_login(
        &self,
        ctx: &Context<'_>,
        session: ID,
        code: String,
    ) -> Result<bool> {
        let code = Zeroizing::new(code);
        let principal = super::graphql::person(ctx)?;
        let session = session.parse().map_err(|_| refused())?;
        if !agent_threads::managed::provider_login::valid_code(&code) {
            return Err(refused());
        }
        let replies = schema_dependency::<Replies>(ctx)?;
        let (device, login) = replies.binding(session, principal).ok_or_else(refused)?;
        let now = schema_dependency::<Arc<dyn Clock>>(ctx)?.now();
        if now >= login.expires_at
            || !authorized(pool(ctx)?, device, &login, now).await
            || !replies.submit(session, principal, code)
        {
            return Err(refused());
        }
        Ok(true)
    }
}
