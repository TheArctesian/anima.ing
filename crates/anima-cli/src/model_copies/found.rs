//! `anima model found`, `anima model find <device>` and `anima model use <device> <model>`: the
//! models that a machine found where Ollama, LM Studio, the Hugging Face cache or a folder of its
//! settings keeps them, and their use where they are (task L1-22; PULL-012, R-444).

use super::documents::{
    FindModels, FoundModels, UseFoundModel, find_models, found_models, use_found_model,
};
use crate::devices::get::resolve;
use crate::failure::{Failure, Hint};
use crate::hub::Hub;
use crate::invocation::Invocation;
use crate::model_library::gib;
use crate::output::Table;
use crate::output::text::enum_name;

pub type Found = found_models::FoundModelsFoundModels;
pub type Used = use_found_model::UseFoundModelUseFoundModel;

#[derive(Debug, clap::Args)]
pub struct FoundArgs {}

#[derive(Debug, clap::Args)]
pub struct FindArgs {
    /// The device's ID or its exact name.
    device: String,
}

#[derive(Debug, clap::Args)]
pub struct UseArgs {
    /// The device's ID or its exact name.
    device: String,
    /// The found model: its key, or the name that its tool shows.
    model: String,
}

/// The found models that the hub lists.
///
/// # Errors
/// The hub's refusal; a missing token.
pub async fn fetch(hub: &Hub) -> Result<Vec<Found>, Failure> {
    Ok(hub
        .run::<FoundModels>(found_models::Variables)
        .await?
        .found_models)
}

/// Asks the machine `device` to read its found models again; gives the job.
///
/// # Errors
/// [`Failure::NotFound`] for an unknown device; the hub's refusal.
pub async fn find(hub: &Hub, device: &str) -> Result<String, Failure> {
    let device_id = resolve(hub, device).await?;
    Ok(hub
        .run::<FindModels>(find_models::Variables { device_id })
        .await?
        .find_models)
}

/// Uses the found model `model` (a key or a shown name) of `device` where it is.
///
/// # Errors
/// [`Failure::NotFound`] for an unknown device or a model that its last report does not list;
/// [`Failure::Usage`] for a name that two of its models show; the hub's refusal.
pub async fn use_found(hub: &Hub, device: &str, model: &str) -> Result<Used, Failure> {
    let device_id = resolve(hub, device).await?;
    let listed = fetch(hub).await?;
    let mut matches = listed.iter().filter(|found| {
        found.place.id == device_id && (found.found == model || found.name == model)
    });
    let Some(first) = matches.next() else {
        return Err(Failure::NotFound {
            message: format!("{device} lists no found model {model}."),
            hint: Some(Hint::run(format!("anima model find {device}"))),
        });
    };
    if matches.next().is_some() {
        return Err(Failure::Usage {
            message: format!("{device} has more than one found model with the name {model}."),
            hint: Some(Hint::run("anima model found".to_owned())),
        });
    }
    let found = first.found.clone();
    Ok(hub
        .run::<UseFoundModel>(use_found_model::Variables { device_id, found })
        .await?
        .use_found_model)
}

impl FoundArgs {
    /// Prints one row for each found model, or with `--json` the K1 `FoundModel` list.
    ///
    /// # Errors
    /// The hub's refusal; a missing token.
    pub async fn run(self, invocation: &Invocation) -> Result<(), Failure> {
        let found = fetch(&invocation.hub()?).await?;
        invocation.output.data(&found, |found| table(found))?;
        if found.is_empty() {
            invocation
                .output
                .note("No machine lists a found model. Read them with anima model find <device>.");
        }
        Ok(())
    }
}

impl FindArgs {
    /// Prints the job that reads the found models, or with `--json` its ID.
    ///
    /// # Errors
    /// [`Failure::NotFound`] for an unknown device; the hub's refusal.
    pub async fn run(self, invocation: &Invocation) -> Result<(), Failure> {
        let job = find(&invocation.hub()?, &self.device).await?;
        invocation.output.data(&job, |job| {
            let mut table = Table::record();
            table.pair("Job", job.as_str());
            table
        })?;
        invocation
            .output
            .note("When the job is done, anima model found lists the models.");
        Ok(())
    }
}

impl UseArgs {
    /// Prints the found copy that the hub asked for, or with `--json` the K1 `ModelCopy`.
    ///
    /// # Errors
    /// As [`use_found`].
    pub async fn run(self, invocation: &Invocation) -> Result<(), Failure> {
        let copy = use_found(&invocation.hub()?, &self.device, &self.model).await?;
        invocation.output.data(&copy, |copy| {
            let mut table = Table::record();
            table.pair("Machine", copy.place.name.as_str());
            table.pair("Model", copy.address.as_str());
            table.pair("Size", gib(copy.size_bytes).as_str());
            table.pair("State", enum_name(&copy.state).as_str());
            table.pair("ID", copy.id.as_str());
            table
        })?;
        invocation.output.note("Follow it with anima model copies.");
        Ok(())
    }
}

fn table(found: &[Found]) -> Table {
    let mut table = Table::new(vec![
        "MACHINE", "TOOL", "NAME", "FORMAT", "SIZE", "USABLE", "KEY",
    ]);
    for model in found {
        table.row(vec![
            model.place.name.as_str().into(),
            enum_name(&model.tool).into(),
            model.name.as_str().into(),
            enum_name(&model.format).into(),
            gib(model.size_bytes).into(),
            if model.usable { "yes" } else { "no" }.into(),
            model.found.as_str().into(),
        ]);
    }
    table
}
