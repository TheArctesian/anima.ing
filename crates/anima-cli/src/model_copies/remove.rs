//! `anima model remove <device> <name:tag>`: removes a copy from its machine (PULL-009). A pull
//! that waits stops and the copy goes; else the machine deletes the files.

use super::copies::fetch;
use super::documents::{ModelCopyRemove, model_copy_remove};
use crate::devices::get::resolve;
use crate::failure::{Failure, Hint};
use crate::invocation::Invocation;
use crate::output::Table;
use crate::output::text::enum_name;

#[derive(Debug, clap::Args)]
pub struct RemoveArgs {
    /// The device's ID or its exact name.
    device: String,
    /// The variant of the copy, name:tag.
    model: String,
}

impl RemoveArgs {
    /// Prints what is left of the copy: nothing, or its state `REMOVING`.
    ///
    /// # Errors
    /// [`Failure::NotFound`] for an unknown device, or a machine that has no copy of the
    /// variant; the hub's refusal.
    pub async fn run(self, invocation: &Invocation) -> Result<(), Failure> {
        let hub = invocation.hub()?;
        let device_id = resolve(&hub, &self.device).await?;
        let copies = fetch(&hub).await?;
        let copy = copies
            .iter()
            .find(|copy| copy.place.id == device_id && copy.address == self.model)
            .ok_or_else(|| Failure::NotFound {
                message: format!("{} has no copy of {}.", self.device, self.model),
                hint: Some(Hint::run("anima model copies".to_owned())),
            })?;
        let left = hub
            .run::<ModelCopyRemove>(model_copy_remove::Variables {
                id: copy.id.clone(),
            })
            .await?
            .remove_model_copy;
        invocation.output.data(&left, |left| {
            let mut table = Table::record();
            table.pair("Address", self.model.as_str());
            let state = left
                .as_ref()
                .map_or_else(|| "Removed".to_owned(), |left| enum_name(&left.state));
            table.pair("State", state.as_str());
            table
        })
    }
}
