//! `anima model pin` and `anima model unpin <device> <name:tag>`: whether a machine keeps a copy
//! loaded, and how long after its last use it unloads it (K11 Section 11.1, FIT-022, FIT-023).

use super::copies::fetch;
use super::documents::{ModelCopyKeep, model_copy_keep};
use crate::devices::get::resolve;
use crate::failure::{Failure, Hint};
use crate::hub::Hub;
use crate::invocation::Invocation;
use crate::output::Table;

#[derive(Debug, clap::Args)]
pub struct KeepArgs {
    /// The device's ID or its exact name.
    device: String,
    /// The variant of the copy, name:tag.
    model: String,
    /// Seconds with no use before the machine unloads the copy, 60 to 86400 (default 900).
    #[arg(long)]
    idle_seconds: Option<u32>,
}

/// What a machine keeps for one copy.
pub struct Keep<'a> {
    pub device: &'a str,
    pub model: &'a str,
    pub pinned: bool,
    pub idle_seconds: Option<u32>,
}

/// Asks the machine of the copy to keep it as `keep` says; gives the job.
///
/// # Errors
/// [`Failure::NotFound`] for an unknown device, or a machine with no copy of the variant; the
/// hub's refusal of a copy that is not on its machine or of seconds out of range.
pub async fn keep(hub: &Hub, keep: &Keep<'_>) -> Result<String, Failure> {
    let device_id = resolve(hub, keep.device).await?;
    let copy = fetch(hub)
        .await?
        .into_iter()
        .find(|copy| copy.place.id == device_id && copy.address == keep.model)
        .ok_or_else(|| Failure::NotFound {
            message: format!("{} has no copy of {}.", keep.device, keep.model),
            hint: Some(Hint::run("anima model copies".to_owned())),
        })?;
    Ok(hub
        .run::<ModelCopyKeep>(model_copy_keep::Variables {
            id: copy.id,
            keep: model_copy_keep::ModelKeepInput {
                pinned: keep.pinned,
                idle_seconds: keep.idle_seconds.map(i64::from),
            },
        })
        .await?
        .keep_model_copy)
}

impl KeepArgs {
    /// Prints the job that carries the keep to the machine.
    ///
    /// # Errors
    /// See [`keep`].
    pub async fn run(self, invocation: &Invocation, pinned: bool) -> Result<(), Failure> {
        let hub = invocation.hub()?;
        let asked = Keep {
            device: &self.device,
            model: &self.model,
            pinned,
            idle_seconds: self.idle_seconds,
        };
        let job = keep(&hub, &asked).await?;
        invocation.output.data(&job, |job| {
            let mut table = Table::record();
            table.pair("Address", self.model.as_str());
            table.pair("Pinned", if pinned { "yes" } else { "no" });
            table.pair("Job", job.as_str());
            table
        })
    }
}
