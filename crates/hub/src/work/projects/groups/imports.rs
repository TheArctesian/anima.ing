//! Automatic migration of device groups (G3, PRO-059).

use async_graphql::{Context, ID, InputObject, Result};
use serde_json::json;

use super::{
    graphql::{checked_name, entry, failed, project_ids},
    store,
};
use crate::{
    api_errors::{FieldError, begin, commit, internal, invalid},
    fleet::events::{AuditAction, Trail},
    work::projects::on,
};

#[derive(InputObject)]
#[graphql(name = "ProjectGroupImportInput")]
pub(super) struct ImportInput {
    /// Stable UUID of the device group; reuse it for every retry and batch.
    source: ID,
    name: String,
    /// At most 200 IDs. Projects absent from this hub are not acknowledged.
    projects: Vec<ID>,
    /// True only for an explicitly empty device group; older hubs reject this field.
    include_empty: Option<bool>,
}

pub(super) async fn import(ctx: &Context<'_>, input: ImportInput) -> Result<Vec<ID>> {
    on(ctx).await?;
    let source = input
        .source
        .parse::<uuid::Uuid>()
        .map_err(|_| invalid(&[FieldError::new("source", "That id is not valid.")]))?;
    let name = checked_name(&input.name)?;
    let projects = project_ids(&input.projects)?;
    let trail = Trail::from_context(ctx)?;
    let mut tx = begin(ctx).await?;
    let known = store::existing(&mut tx, &projects).await.map_err(failed)?;
    if known.is_empty() && (!projects.is_empty() || input.include_empty != Some(true)) {
        commit(tx).await?;
        return Ok(Vec::new());
    }
    let (group, first) = store::import_group(&mut tx, source, &name, trail.now)
        .await
        .map_err(failed)?;
    let fresh = store::import_members(&mut tx, source, &known)
        .await
        .map_err(failed)?;
    // A deleted destination remains a receipt, not permission to recreate the group.
    if store::lock(&mut tx, group).await.map_err(failed)? {
        let moved = store::import_unassigned(&mut tx, &fresh, group)
            .await
            .map_err(failed)?;
        if first || !fresh.is_empty() {
            trail
                .write(
                    &mut tx,
                    entry(
                        AuditAction::Update,
                        group,
                        "Device project groups were shared.",
                        json!({ "projects": moved }),
                    ),
                )
                .await
                .map_err(internal)?;
        }
    }
    commit(tx).await?;
    Ok(known.into_iter().map(|id| ID(id.to_string())).collect())
}
