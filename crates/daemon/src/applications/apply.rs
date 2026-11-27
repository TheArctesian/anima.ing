//! Deliver complete application inputs over the authenticated owner connection.
use anima_applications_wire::{
    Command, Decision, MAX_COMPOSE_BYTES, MAX_ENVIRONMENT_BYTES, MAX_REPLY_BYTES, Reply,
};
use chrono::Utc;
use daemon_core::wire::Reported;
use serde_json::Value;
use tokio::{net::UnixStream, time};

use crate::hub::work::ReleasedSecret;

use super::{CONNECT, Client, Error, PREPARATION, frame, read, send};

pub(crate) struct ApplyFiles {
    pub compose: ReleasedSecret,
    pub environment: Option<ReleasedSecret>,
}

impl ApplyFiles {
    pub(crate) fn live(&self) -> bool {
        self.compose.live() && self.environment.as_ref().is_none_or(ReleasedSecret::live)
    }
}

pub(crate) struct StagedApply(UnixStream);

impl Client {
    pub(crate) async fn supports_apply(&self) -> bool {
        matches!(
            self.exchange(Command::ProbeApply, CONNECT).await,
            Ok((_, Reply::ApplyAvailable))
        )
    }

    pub(crate) async fn stage_apply(
        &self,
        item: &Value,
        files: ApplyFiles,
    ) -> Result<StagedApply, &'static str> {
        if !files.live() {
            return Err("secret_failed");
        }
        let (mut stream, reply) = self.offer(item).await.map_err(|_| "applications_off")?;
        if reply != Reply::NeedsFiles {
            return Err(reason(reply));
        }
        if !files.live() {
            return Err("secret_failed");
        }
        let reply = time::timeout(PREPARATION, async {
            frame(&mut stream, files.compose.bytes(), MAX_COMPOSE_BYTES).await?;
            if let Some(environment) = &files.environment {
                frame(&mut stream, environment.bytes(), MAX_ENVIRONMENT_BYTES).await?;
            }
            drop(files);
            read(&mut stream).await
        })
        .await
        .map_err(|_| "step_failed")?
        .map_err(|_| "step_failed")?;
        match reply {
            Reply::Ready => Ok(StagedApply(stream)),
            other => Err(reason(other)),
        }
    }
}

impl StagedApply {
    pub(crate) async fn start(mut self, ack: Reported) -> Result<UnixStream, Error> {
        if ack.stop.is_some() || ack.lease.until <= Utc::now() {
            return Err(Error::NotAuthorized);
        }
        time::timeout(
            CONNECT,
            send(&mut self.0, &Decision::Start, MAX_REPLY_BYTES),
        )
        .await
        .map_err(|_| Error::Connection)??;
        Ok(self.0)
    }
}

pub(crate) fn reason(reply: Reply) -> &'static str {
    let (Reply::Failed { cause } | Reply::Refused { cause }) = reply else {
        return "step_failed";
    };
    match cause.as_str() {
        "applications_off" => "applications_off",
        "busy" => "busy",
        "file_not_permitted" => "file_not_permitted",
        "limit_disk" => "limit_disk",
        "limit_seconds" => "limit_seconds",
        "name_taken" => "name_taken",
        "migration_held" => "migration_held",
        "no_digest" => "no_digest",
        "not_at_machine" => "not_at_machine",
        "orphan" => "orphan",
        "program_mismatch" => "program_mismatch",
        "protected" => "protected",
        _ => "step_failed",
    }
}
