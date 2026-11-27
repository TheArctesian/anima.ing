//! One-use browser handoff and opaque host-only session credentials.
use super::{Error, Result, previews};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use chrono::{DateTime, TimeDelta, Utc};
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use uuid::Uuid;

fn secret() -> Result<String> {
    let mut bytes = [0_u8; 32];
    getrandom::fill(&mut bytes)
        .map_err(|_| Error::Refused("Preview session entropy is unavailable."))?;
    Ok(URL_SAFE_NO_PAD.encode(bytes))
}
fn digest(token: &str) -> Result<Vec<u8>> {
    if token.len() != 43
        || !URL_SAFE_NO_PAD
            .decode(token)
            .is_ok_and(|bytes| bytes.len() == 32)
    {
        return Err(Error::Refused("Preview session is unavailable."));
    }
    Ok(Sha256::digest(token.as_bytes()).to_vec())
}

/// Browser cookies and forwarding credentials have disjoint hash domains.
#[derive(Clone, Copy)]
pub(in crate::fleet) enum Audience {
    Browser,
    Forward,
}

impl Audience {
    fn session_hash(self, token: &str) -> Result<Vec<u8>> {
        let browser_hash = digest(token)?;
        match self {
            Self::Browser => Ok(browser_hash),
            Self::Forward => {
                let mut hash = Sha256::new();
                hash.update(b"anima-workspace-forward.v1");
                hash.update(browser_hash);
                Ok(hash.finalize().to_vec())
            }
        }
    }

    /// Redeeming consumes the handoff and returns a credential for the chosen audience.
    pub(in crate::fleet) async fn redeem(
        self,
        pool: &PgPool,
        (endpoint, token): (Uuid, &str),
        now: DateTime<Utc>,
    ) -> Result<(String, i64)> {
        let credential = secret()?;
        let mut tx = pool.begin().await?;
        let held: Option<(Uuid, DateTime<Utc>)> = sqlx::query_as("UPDATE agent_threads.preview_sessions s SET redeemed=true,session_hash=$3
        FROM agent_threads.previews p, agent_threads.workspaces w
        WHERE s.handoff_hash=$1 AND s.endpoint=$2 AND NOT s.redeemed AND s.handoff_expires > $4 AND s.expires_at > $4
          AND p.endpoint=s.endpoint AND p.principal=s.principal AND NOT p.revoked AND p.expires_at > $4
          AND w.device_id=p.device_id AND w.workspace=p.workspace AND w.generation=p.generation
          AND w.ready AND w.enabled AND w.seen_at >= $4 - interval '30 seconds'
        RETURNING s.principal,s.expires_at")
        .bind(digest(token)?).bind(endpoint).bind(self.session_hash(&credential)?).bind(now).fetch_optional(&mut *tx).await?;
        let (_, expires) = held.ok_or(Error::Refused(
            "Preview handoff is unavailable or already used.",
        ))?;
        tx.commit().await?;
        Ok((credential, (expires - now).num_seconds().max(0)))
    }

    pub(in crate::fleet) async fn principal(
        self,
        pool: &PgPool,
        (endpoint, token): (Uuid, &str),
        now: DateTime<Utc>,
    ) -> Result<Uuid> {
        let principal = sqlx::query_scalar(
            "SELECT principal FROM agent_threads.preview_sessions
        WHERE session_hash=$1 AND endpoint=$2 AND redeemed AND expires_at > $3",
        )
        .bind(self.session_hash(token)?)
        .bind(endpoint)
        .bind(now)
        .fetch_optional(pool)
        .await?
        .ok_or(Error::Refused("Preview session expired."))?;
        previews::authorized(pool, endpoint, principal, now).await?;
        Ok(principal)
    }
}

pub(super) async fn issue(
    pool: &PgPool,
    endpoint: Uuid,
    principal: Uuid,
    now: DateTime<Utc>,
) -> Result<String> {
    let (_, grant) = previews::authorized(pool, endpoint, principal, now).await?;
    let expires = DateTime::from_timestamp(i64::try_from(grant.binding.expires_at)?, 0)
        .ok_or(Error::Refused("Invalid preview expiry."))?;
    let mut tx = pool.begin().await?;
    sqlx::query("SELECT endpoint FROM agent_threads.previews WHERE endpoint=$1 FOR UPDATE")
        .bind(endpoint)
        .execute(&mut *tx)
        .await?;
    let count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM agent_threads.preview_sessions WHERE endpoint=$1")
            .bind(endpoint)
            .fetch_one(&mut *tx)
            .await?;
    if count >= 8 {
        return Err(Error::Refused(
            "Open a new preview after this preview's browser-session limit.",
        ));
    }
    let token = secret()?;
    sqlx::query("INSERT INTO agent_threads.preview_sessions (handoff_hash,endpoint,principal,handoff_expires,expires_at)
        VALUES ($1,$2,$3,$4,$5)")
        .bind(digest(&token)?).bind(endpoint).bind(principal).bind((now+TimeDelta::seconds(60)).min(expires)).bind(expires)
        .execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(token)
}
