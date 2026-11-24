//! `anima model copies`: each copy that a machine has, pulls or removes (PULL-007).

use super::documents::{ModelCopies, model_copies};
use super::state_cell;
use crate::failure::Failure;
use crate::hub::Hub;
use crate::invocation::Invocation;
use crate::model_library::gib;
use crate::output::Table;
use crate::output::text::enum_name;

pub type Copy = model_copies::ModelCopiesModelCopies;

#[derive(Debug, clap::Args)]
pub struct CopiesArgs {}

/// The copies that the hub lists.
///
/// # Errors
/// The hub's refusal; a missing token.
pub async fn fetch(hub: &Hub) -> Result<Vec<Copy>, Failure> {
    Ok(hub
        .run::<ModelCopies>(model_copies::Variables)
        .await?
        .model_copies)
}

impl CopiesArgs {
    /// Prints one row for each copy, or with `--json` the K1 `ModelCopy` list.
    ///
    /// # Errors
    /// The hub's refusal; a missing token.
    pub async fn run(self, invocation: &Invocation) -> Result<(), Failure> {
        let hub = invocation.hub()?;
        let copies = fetch(&hub).await?;
        invocation.output.data(&copies, |copies| table(copies))?;
        if copies.is_empty() {
            invocation.output.note(
                "No machine has a model. Pull one with anima model pull <device> <name:tag>.",
            );
        }
        Ok(())
    }
}

fn table(copies: &[Copy]) -> Table {
    let mut table = Table::new(vec![
        "MACHINE", "ADDRESS", "SIZE", "STATE", "LOAD", "REASON", "ID",
    ]);
    for copy in copies {
        let progress = copy
            .progress
            .as_ref()
            .map(|each| (each.bytes, each.total_bytes, each.seconds_left));
        table.row(vec![
            copy.place.name.as_str().into(),
            copy.address.as_str().into(),
            gib(copy.size_bytes).into(),
            state_cell(&copy.state, progress).into(),
            load_cell(copy.load.as_ref()).into(),
            copy.reason.as_deref().unwrap_or("").into(),
            copy.id.as_str().into(),
        ]);
    }
    table
}

/// Where the copy is in the memory of its machine: its state, and "pinned" for a pinned copy.
fn load_cell(load: Option<&model_copies::ModelCopiesModelCopiesLoad>) -> String {
    let Some(load) = load else {
        return String::new();
    };
    let state = enum_name(&load.state);
    if load.pinned {
        format!("{state}, pinned")
    } else {
        state
    }
}
