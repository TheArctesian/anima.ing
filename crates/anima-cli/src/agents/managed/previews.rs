//! Human-owned preview grants. Browser handoff secrets stay in the Code app.
use super::{
    cell,
    documents::{
        ManagedPreviewClose, ManagedPreviewOpen, ManagedPreviews, managed_preview_close,
        managed_preview_open, managed_previews,
    },
    record,
};
use crate::{devices::get::resolve, failure::Failure, invocation::Invocation, output::Table};
use serde_json::json;
use uuid::Uuid;

#[derive(Debug, clap::Subcommand)]
pub enum Command {
    /// List this person's live grants for a managed workspace.
    List { device: String, workspace: Uuid },
    /// Grant ten minutes of access to a workspace loopback port, without opening a browser.
    Create {
        device: String,
        workspace: Uuid,
        #[arg(long)]
        generation: u64,
        #[arg(long, default_value_t = 3000, value_parser = clap::value_parser!(u16).range(1..))]
        port: u16,
        /// Reuse only for the same destination after an unconfirmed submission.
        #[arg(long)]
        endpoint: Option<Uuid>,
    },
    /// Revoke this person's grant and its active preview connections.
    Close { endpoint: Uuid },
}

impl Command {
    pub async fn run(self, invocation: &Invocation) -> Result<(), Failure> {
        let hub = invocation.hub()?;
        match self {
            Self::List { device, workspace } => {
                let device = resolve(&hub, &device).await?;
                let rows = hub
                    .run::<ManagedPreviews>(managed_previews::Variables {
                        device,
                        workspace: workspace.to_string(),
                    })
                    .await?
                    .managed_code_previews;
                invocation.output.data(&rows, |rows| {
                    let mut table = Table::new(vec!["ENDPOINT", "GENERATION", "PORT", "EXPIRES"]);
                    for row in rows {
                        table.row(
                            ["endpoint", "generation", "port", "expiresAt"]
                                .into_iter()
                                .map(|key| cell(&row[key]))
                                .collect(),
                        );
                    }
                    table
                })
            }
            Self::Create {
                device,
                workspace,
                generation,
                port,
                endpoint,
            } => {
                let device = resolve(&hub, &device).await?;
                let endpoint = endpoint.unwrap_or_else(Uuid::now_v7);
                let reply = hub.run::<ManagedPreviewOpen>(managed_preview_open::Variables {
                    device,
                    input: managed_preview_open::ManagedCodePreviewInput {
                        endpoint: endpoint.to_string(), workspace: workspace.to_string(),
                        generation: generation.to_string(), port: i64::from(port),
                    },
                }).await.map_err(|error| Failure::NotDelivered {
                    message: format!("Preview grant was not confirmed: {error}. List this workspace's previews; retry only the same destination with --endpoint {endpoint}."),
                })?;
                invocation
                    .output
                    .data(&reply.open_managed_code_preview, record)
            }
            Self::Close { endpoint } => {
                let closed = hub
                    .run::<ManagedPreviewClose>(managed_preview_close::Variables {
                        endpoint: endpoint.to_string(),
                    })
                    .await?
                    .close_managed_code_preview;
                invocation
                    .output
                    .data(&json!({"endpoint": endpoint, "revoked": closed}), record)
            }
        }
    }
}
