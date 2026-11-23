//! `anima model pull <device> <name:tag>`: the machine downloads the variant from its source
//! and verifies it (PULL-001). The hub selects no variant: a name with no tag is refused with
//! the tags of the model (PULL-002 waits for task L1-8).

use super::documents::{ModelPull, model_pull};
use crate::devices::get::resolve;
use crate::failure::Failure;
use crate::invocation::Invocation;
use crate::model_library::gib;
use crate::output::Table;
use crate::output::text::enum_name;

#[derive(Debug, clap::Args)]
pub struct PullArgs {
    /// The device's ID or its exact name.
    device: String,
    /// The variant, name:tag, such as qwen3:8b-q4_k_m.
    model: String,
}

impl PullArgs {
    /// Prints the copy that the hub asked for, or with `--json` the K1 `ModelCopy`.
    ///
    /// # Errors
    /// [`Failure::NotFound`] for an unknown device; the hub's refusal (`VALIDATION` for a
    /// missing tag, or a copy that the machine has, pulls or removes).
    pub async fn run(self, invocation: &Invocation) -> Result<(), Failure> {
        let hub = invocation.hub()?;
        let device_id = resolve(&hub, &self.device).await?;
        let copy = hub
            .run::<ModelPull>(model_pull::Variables {
                device_id,
                model: self.model,
            })
            .await?
            .pull_model;
        invocation.output.data(&copy, |copy| {
            let mut table = Table::record();
            table.pair("Machine", copy.place.name.as_str());
            table.pair("Address", copy.address.as_str());
            table.pair("Size", gib(copy.size_bytes).as_str());
            table.pair("State", enum_name(&copy.state).as_str());
            table.pair("ID", copy.id.as_str());
            table
        })?;
        invocation.output.note("Follow it with anima model copies.");
        Ok(())
    }
}
