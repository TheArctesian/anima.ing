//! The copies GraphQL part: `Query.modelCopies`, `Mutation.pullModel` and
//! `Mutation.removeModelCopy` (K1; task L1-7; PULL-001, PULL-007, PULL-009). Each mutation
//! writes the copy, its job, the audit entry of the job and one audit entry and event of the
//! copy in one transaction. An API key or an agent may ask, with its actor (R-326).

mod load;
mod types;

use std::collections::HashMap;

use async_graphql::{Context, Error as GraphQlError, ID, Object, Result};
use serde_json::json;

pub use load::ModelLoad;
pub use types::{FoundModelView, FoundTool, HostPlace, HostPlaceKind, ModelCopyView, PullProgress};

use super::{Copy, Error, Listed};
use crate::{
    api_errors::{
        ErrorCode, FieldError, begin, coded, commit, internal, invalid, pool, schema_dependency,
    },
    fleet::{
        access::Authenticated,
        devices::{self, Device},
        events::{
            AuditAction, AuditTarget, ClientAddr, Operator, Severity, Source, Trail, TrailEntry,
        },
    },
    ids::{CopyId, DeviceId},
    work::jobs::{self, Asker},
};

fn to_graphql(error: Error) -> GraphQlError {
    use super::super::library::{self, Missing};
    let on_model = |words: &str| invalid(&[FieldError::new("model", words)]);
    match error {
        Error::NotFound => coded("No such model copy.", ErrorCode::NotFound),
        Error::FoundUnknown => coded(
            "The last model report of that machine lists no such model. Read its models again.",
            ErrorCode::NotFound,
        ),
        Error::Library(library::Error::NotFound(Missing::Model)) => {
            coded("No such model.", ErrorCode::NotFound)
        }
        Error::Library(library::Error::Validation(fields)) => invalid(&fields),
        Error::Unpullable(why) => on_model(why),
        Error::Taken(state) => on_model(&format!(
            "That machine already has this variant ({}). Remove it first to pull it again.",
            types::state_words(state)
        )),
        Error::NotOnMachine(state) => invalid(&[FieldError::new(
            "id",
            format!(
                "The machine does not have this copy now ({}).",
                types::state_words(state)
            ),
        )]),
        Error::Jobs(jobs::Error::NoFit(reason)) => invalid(&[FieldError::new("deviceId", reason)]),
        other => internal(other),
    }
}

fn asker(ctx: &Context<'_>) -> Result<Asker> {
    Ok((
        schema_dependency::<Operator>(ctx)?.clone(),
        *schema_dependency::<ClientAddr>(ctx)?,
    ))
}

async fn device_of(ctx: &Context<'_>, id: &ID) -> Result<Device> {
    let id: DeviceId = id
        .parse()
        .map_err(|_| invalid(&[FieldError::new("deviceId", "That id is not valid.")]))?;
    devices::get(pool(ctx)?, id)
        .await
        .map_err(|error| match error {
            devices::Error::NotFound(_) => coded("No such device.", ErrorCode::NotFound),
            other => internal(other),
        })
}

fn entry(device: &Device, copy: &Copy, (action, message): (AuditAction, String)) -> TrailEntry {
    TrailEntry {
        source: Source::Device,
        severity: Severity::Info,
        device: Some((device.id, device.name.clone())),
        message,
        action,
        target: AuditTarget::Device(device.id),
        change: json!({ "modelCopy": {
            "copy": copy.id, "model": copy.address(), "revision": copy.revision,
        } }),
    }
}

#[derive(Default)]
pub struct Query;

#[Object(name = "ModelCopiesQuery")]
impl Query {
    /// Each copy that a machine has, pulls or removes, in the order of the machine, the model
    /// and the tag; each names its place (HOST-052).
    #[graphql(guard = "Authenticated")]
    async fn model_copies(&self, ctx: &Context<'_>) -> Result<Vec<ModelCopyView>> {
        let db = pool(ctx)?;
        let listed = super::list(db).await.map_err(to_graphql)?;
        let names = device_names(db).await?;
        let mut loads = super::loads::by_copy(db).await.map_err(to_graphql)?;
        Ok(listed
            .into_iter()
            .map(|listed| {
                let name = names.get(&listed.copy.device).cloned().unwrap_or_default();
                let load = loads.remove(&(listed.copy.device, listed.copy.id));
                ModelCopyView::new(listed, name).with_load(load)
            })
            .collect())
    }

    /// Each model that a machine found where a different program keeps it (Ollama, LM Studio,
    /// the Hugging Face cache, a folder of the owner), from its last model report, with no copy
    /// that uses it (PULL-012), in the order of the machine and the name.
    #[graphql(guard = "Authenticated")]
    async fn found_models(&self, ctx: &Context<'_>) -> Result<Vec<FoundModelView>> {
        let db = pool(ctx)?;
        let found = super::found::list(db).await.map_err(to_graphql)?;
        let names = device_names(db).await?;
        Ok(found
            .into_iter()
            .map(|found| {
                let name = names.get(&found.device).cloned().unwrap_or_default();
                FoundModelView::new(found, name)
            })
            .collect())
    }
}

async fn device_names(db: &sqlx::PgPool) -> Result<HashMap<DeviceId, String>> {
    Ok(devices::list(db)
        .await
        .map_err(internal)?
        .into_iter()
        .map(|device| (device.id, device.name))
        .collect())
}

/// What a copy keeps on its machine (K11 Section 11.1).
#[derive(async_graphql::InputObject)]
pub struct ModelKeepInput {
    /// A pinned copy stays loaded, and comes back when other work took its memory (FIT-023).
    pinned: bool,
    /// The seconds with no use before the machine unloads the copy, from 60 to 86400; 900 when not given (FIT-022).
    idle_seconds: Option<u32>,
}

#[derive(Default)]
pub struct Mutation;

#[Object(name = "ModelCopiesMutation")]
impl Mutation {
    /// Pulls the variant `model:tag` to the machine (PULL-001). A name with no tag is refused
    /// with the tags of the model (PULL-002 waits for task L1-8). A copy that failed is pulled
    /// again; a copy that the machine has, pulls or removes is refused.
    #[graphql(guard = "Authenticated")]
    async fn pull_model(
        &self,
        ctx: &Context<'_>,
        device_id: ID,
        model: String,
    ) -> Result<ModelCopyView> {
        let device = device_of(ctx, &device_id).await?;
        let source = super::source_of(pool(ctx)?, &model)
            .await
            .map_err(to_graphql)?;
        let asker = asker(ctx)?;
        let trail = Trail::from_context(ctx)?;
        let now = trail.now;
        let mut tx = begin(ctx).await?;
        let copy = super::pull(&mut tx, (device.id, &source), asker, now)
            .await
            .map_err(to_graphql)?;
        let message = format!("Asked {} to pull {}.", device.name, copy.address());
        trail
            .write(
                &mut tx,
                entry(&device, &copy, (AuditAction::Create, message)),
            )
            .await
            .map_err(internal)?;
        let listed = super::listed(&mut tx, copy).await.map_err(to_graphql)?;
        commit(tx).await?;
        let listed = listed.ok_or_else(|| coded("No such model copy.", ErrorCode::NotFound))?;
        Ok(ModelCopyView::new(listed, device.name))
    }

    /// Asks the machine for its model state, so that its found models are read again
    /// (PULL-012). Gives the job; `foundModels` lists the models when it is done.
    #[graphql(guard = "Authenticated")]
    async fn find_models(&self, ctx: &Context<'_>, device_id: ID) -> Result<ID> {
        let device = device_of(ctx, &device_id).await?;
        let asker = asker(ctx)?;
        let now = Trail::from_context(ctx)?.now;
        let mut tx = begin(ctx).await?;
        let job = super::found::read(&mut tx, device.id, asker, now)
            .await
            .map_err(to_graphql)?;
        commit(tx).await?;
        Ok(ID(job.to_string()))
    }

    /// Uses a found model where it is (PULL-012, R-444): the machine reads each of its files
    /// and keeps their sizes and SHA-256, and checks them again at each load. A model that is
    /// not GGUF is refused; a found copy that failed is asked for again.
    #[graphql(guard = "Authenticated")]
    async fn use_found_model(
        &self,
        ctx: &Context<'_>,
        device_id: ID,
        found: String,
    ) -> Result<ModelCopyView> {
        if !daemon_core::model_work::names::is_lower_hex(&found, 64) {
            return Err(invalid(&[FieldError::new(
                "found",
                "That key is not valid.",
            )]));
        }
        let device = device_of(ctx, &device_id).await?;
        let asker = asker(ctx)?;
        let trail = Trail::from_context(ctx)?;
        let now = trail.now;
        let mut tx = begin(ctx).await?;
        let copy = super::found::adopt(&mut tx, (device.id, &found), asker, now)
            .await
            .map_err(to_graphql)?;
        let message = format!(
            "Asked {} to use {} where it is.",
            device.name,
            copy.address()
        );
        let audit = entry(&device, &copy, (AuditAction::Create, message));
        trail.write(&mut tx, audit).await.map_err(internal)?;
        let listed = super::listed(&mut tx, copy).await.map_err(to_graphql)?;
        commit(tx).await?;
        let listed = listed.ok_or_else(|| coded("No such model copy.", ErrorCode::NotFound))?;
        Ok(ModelCopyView::new(listed, device.name))
    }

    /// Keeps the copy loaded on its machine for `idleSeconds` after its last use (default 900), and pins it or
    /// ends its pin (FIT-022, FIT-023; K11 Section 11.1). Gives the job of the machine.
    #[graphql(guard = "Authenticated")]
    async fn keep_model_copy(&self, ctx: &Context<'_>, id: ID, keep: ModelKeepInput) -> Result<ID> {
        use daemon_core::model_work::memory::keep::{DEFAULT_IDLE_SECONDS, IDLE_SECONDS, Keep};
        let ModelKeepInput {
            pinned,
            idle_seconds,
        } = keep;
        let id: CopyId = id
            .parse()
            .map_err(|_| invalid(&[FieldError::new("id", "That id is not valid.")]))?;
        let idle_seconds = idle_seconds.unwrap_or(DEFAULT_IDLE_SECONDS);
        if !IDLE_SECONDS.contains(&idle_seconds) {
            let words = "Give from 60 seconds to 86400 seconds (one day).";
            return Err(invalid(&[FieldError::new("idleSeconds", words)]));
        }
        let keep = Keep {
            idle_seconds,
            pinned,
        };
        let asker = asker(ctx)?;
        let trail = Trail::from_context(ctx)?;
        let now = trail.now;
        let mut tx = begin(ctx).await?;
        let (copy, job) = super::loads::keep(&mut tx, (id, keep), asker, now)
            .await
            .map_err(to_graphql)?;
        let device = devices::get(pool(ctx)?, copy.device)
            .await
            .map_err(internal)?;
        let pin = if pinned { "pin" } else { "not pin" };
        let message = format!(
            "Asked {} to {pin} {} and keep it loaded for {idle_seconds} seconds after its last use.",
            device.name,
            copy.address()
        );
        let audit = entry(&device, &copy, (AuditAction::Update, message));
        trail.write(&mut tx, audit).await.map_err(internal)?;
        commit(tx).await?;
        Ok(ID(job.to_string()))
    }

    /// Removes the copy (PULL-009): a pull that waits stops and the copy goes (null); else the
    /// machine deletes its files, and the copy is `REMOVING` until that is done.
    #[graphql(guard = "Authenticated")]
    async fn remove_model_copy(&self, ctx: &Context<'_>, id: ID) -> Result<Option<ModelCopyView>> {
        let id: CopyId = id
            .parse()
            .map_err(|_| invalid(&[FieldError::new("id", "That id is not valid.")]))?;
        let asker = asker(ctx)?;
        let trail = Trail::from_context(ctx)?;
        let now = trail.now;
        let mut tx = begin(ctx).await?;
        let removed = super::remove(&mut tx, id, asker, now)
            .await
            .map_err(to_graphql)?;
        let device = devices::get(pool(ctx)?, removed.copy.device)
            .await
            .map_err(internal)?;
        let message = format!(
            "Asked {} to remove {}.",
            device.name,
            removed.copy.address()
        );
        let audit = entry(&device, &removed.copy, (AuditAction::Delete, message));
        trail.write(&mut tx, audit).await.map_err(internal)?;
        let listed: Option<Listed> = match removed.kept {
            Some(kept) => super::listed(&mut tx, kept).await.map_err(to_graphql)?,
            None => None,
        };
        commit(tx).await?;
        Ok(listed.map(|listed| ModelCopyView::new(listed, device.name)))
    }
}
