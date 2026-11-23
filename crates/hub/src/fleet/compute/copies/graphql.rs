//! The copies GraphQL part: `Query.modelCopies`, `Mutation.pullModel` and
//! `Mutation.removeModelCopy` (K1; task L1-7; PULL-001, PULL-007, PULL-009). Each mutation
//! writes the copy, its job, the audit entry of the job and one audit entry and event of the
//! copy in one transaction. An API key or an agent may ask, with its actor (R-326).

mod types;

use std::collections::HashMap;

use async_graphql::{Context, Error as GraphQlError, ID, Object, Result};
use serde_json::json;

pub use types::{HostPlace, HostPlaceKind, ModelCopyView, PullProgress};

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
        Error::Library(library::Error::NotFound(Missing::Model)) => {
            coded("No such model.", ErrorCode::NotFound)
        }
        Error::Library(library::Error::Validation(fields)) => invalid(&fields),
        Error::Unpullable(why) => on_model(why),
        Error::Taken(state) => on_model(&format!(
            "That machine already has this variant ({}). Remove it first to pull it again.",
            types::state_words(state)
        )),
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
        let names: HashMap<DeviceId, String> = devices::list(db)
            .await
            .map_err(internal)?
            .into_iter()
            .map(|device| (device.id, device.name))
            .collect();
        Ok(listed
            .into_iter()
            .map(|listed| {
                let name = names.get(&listed.copy.device).cloned().unwrap_or_default();
                ModelCopyView::new(listed, name)
            })
            .collect())
    }
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
