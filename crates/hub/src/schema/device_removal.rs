use async_graphql::{Context, ID, Object, Result};

use crate::{
    api_errors::{begin, commit, internal, invalid, pool, schema_dependency},
    fleet::{
        access::Authenticated,
        devices::graphql::{device_id, stage_removal},
        events::{ClientAddr, Operator, Trail},
        sessions::{Actor, DeviceSessions, Live},
    },
};

#[derive(Default)]
pub struct DeviceRemoval;

#[Object(name = "DeviceRemovalMutation")]
impl DeviceRemoval {
    #[graphql(guard = "Authenticated")]
    async fn delete_device(&self, ctx: &Context<'_>, id: ID) -> Result<ID> {
        let id = device_id(&id).map_err(|error| invalid(&[error]))?;
        let trail = Trail::from_context(ctx)?;
        let by = Actor {
            operator: schema_dependency::<Operator>(ctx)?.clone(),
            client: *schema_dependency::<ClientAddr>(ctx)?,
            at: trail.now,
        };
        let db = pool(ctx)?.clone();
        let live = schema_dependency::<Live>(ctx)?.clone();
        let mut tx = begin(ctx).await?;
        let sessions = DeviceSessions::prepare(&mut tx, id)
            .await
            .map_err(internal)?;
        stage_removal(ctx, &mut tx, id, trail).await?;
        // Disconnect even if the client drops the request while the commit is in progress.
        tokio::spawn(async move {
            commit(tx).await?;
            sessions.close(&db, &live, &by).await;
            Ok(ID(id.to_string()))
        })
        .await
        .map_err(internal)?
    }
}
