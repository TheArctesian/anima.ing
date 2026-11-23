//! SEL-005, PULL-001, PULL-007, PULL-009, CMD-030; the server cleans and caps each string from
//! Hugging Face (MCP-008). A pull and a removal are open to an API key; the hub records the
//! actor (R-326).

use rmcp::model::JsonObject;
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::hub::Hub;
use crate::mcp::arguments::{json, object, parse, short};
use crate::mcp::catalogue::Access;
use crate::mcp::definition::{Definition, Effect};
use crate::mcp::outcome::ToolError;
use crate::devices::get::resolve;
use crate::model_copies::copies::fetch;
use crate::model_copies::documents::{ModelCopyRemove, ModelPull, model_copy_remove, model_pull};
use crate::model_library::documents::{ModelList, ModelShow, model_list, model_show};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModelTool {
    List,
    Show,
    Copies,
    Pull,
    Remove,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ListInput {}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ShowInput {
    /// The model's name, an alias, or name:tag.
    name: String,
    /// Add the file list of each variant with its SHA-256 values.
    #[serde(default)]
    detail: bool,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CopyInput {
    /// The device's ID or its exact name.
    device: String,
    /// The variant, name:tag, such as qwen3:8b-q4_k_m.
    model: String,
}

const VARIANT_KEYS: [&str; 6] = [
    "address",
    "tag",
    "format",
    "quantisation",
    "sourceRepository",
    "sizeBytes",
];

impl ModelTool {
    pub const ALL: [Self; 5] = [
        Self::List,
        Self::Show,
        Self::Copies,
        Self::Pull,
        Self::Remove,
    ];

    #[must_use]
    pub fn definition(self) -> Definition {
        match self {
            Self::List => Definition::new::<ListInput>(
                "anima_model_list",
                Access::Read,
                Effect::Reads,
                "The models of the library in name order: name, family, parameter count, \
                 context length, capability tags, licence, aliases, and each variant by its \
                 address name:tag with its format, quantisation and size. A tag says what \
                 a model can do, not that a device runs it.",
            )
            .returns(&[("models", "array", "The models of the library.")]),
            Self::Show => Definition::new::<ShowInput>(
                "anima_model_show",
                Access::Read,
                Effect::Reads,
                "One model by its name, alias or name:tag: its facts (layers, hidden size, \
                 heads), its publisher and source repositories, and its variants; with \
                 detail, each file with its size and SHA-256 value. Null when there is no \
                 such model.",
            )
            .returns(&[("model", "object", "The model; null when unknown.")]),
            Self::Copies => Definition::new::<ListInput>(
                "anima_model_copies",
                Access::Read,
                Effect::Reads,
                "Each copy of a model that a machine has, pulls or removes: its machine, its \
                 address name:tag, its size, its state (QUEUED, DOWNLOADING, DOWNLOADED, \
                 VALIDATING, VALIDATED, FAILED, REMOVING), the progress of a running pull and \
                 the reason of a failure. Next: anima_model_pull, anima_model_remove.",
            )
            .returns(&[("copies", "array", "The copies.")]),
            Self::Pull => Definition::new::<CopyInput>(
                "anima_model_pull",
                Access::Write,
                Effect::Adds,
                "Asks one machine to download a variant name:tag of the library from its source \
                 and verify each file. A name with no tag is refused with the tags of the model. \
                 A machine that has the variant, or pulls or removes it, refuses a second pull. \
                 Returns the copy; follow it with anima_model_copies.",
            )
            .returns(&[("copy", "object", "The copy that the hub asked for.")]),
            Self::Remove => Definition::new::<CopyInput>(
                "anima_model_remove",
                Access::Write,
                Effect::Removes,
                "Removes the copy of a variant name:tag from one machine. A pull that waits \
                 stops and the copy goes at once; else the machine deletes the files. Returns \
                 the copy in state REMOVING, or null when it is gone.",
            )
            .returns(&[("copy", "object", "What is left of the copy; null when gone.")]),
        }
    }

    /// Runs the tool.
    ///
    /// # Errors
    /// [`ToolError`] for wrong arguments or the hub's refusal.
    pub async fn call(self, hub: &Hub, arguments: Option<JsonObject>) -> Result<Value, ToolError> {
        match self {
            Self::List => list(hub, arguments).await,
            Self::Show => show(hub, arguments).await,
            Self::Copies => copies(hub, arguments).await,
            Self::Pull => pull(hub, arguments).await,
            Self::Remove => remove(hub, arguments).await,
        }
    }
}

async fn list(hub: &Hub, arguments: Option<JsonObject>) -> Result<Value, ToolError> {
    let _: ListInput = parse(arguments)?;
    let models = hub
        .run::<ModelList>(model_list::Variables)
        .await?
        .library_models;
    Ok(Value::Object(object([("models", json(&models))])))
}

async fn show(hub: &Hub, arguments: Option<JsonObject>) -> Result<Value, ToolError> {
    let input: ShowInput = parse(arguments)?;
    let answer = hub
        .run::<ModelShow>(model_show::Variables { name: input.name })
        .await?;
    let mut model = json(&answer.library_model);
    if let Some(variants) = model.get_mut("variants").and_then(Value::as_array_mut) {
        for variant in variants.iter_mut() {
            *variant = short(variant.take(), &VARIANT_KEYS, input.detail);
        }
    }
    Ok(json!({ "model": model }))
}

async fn copies(hub: &Hub, arguments: Option<JsonObject>) -> Result<Value, ToolError> {
    let _: ListInput = parse(arguments)?;
    Ok(json!({ "copies": json(&fetch(hub).await?) }))
}

async fn pull(hub: &Hub, arguments: Option<JsonObject>) -> Result<Value, ToolError> {
    let input: CopyInput = parse(arguments)?;
    let device_id = resolve(hub, &input.device).await?;
    let copy = hub
        .run::<ModelPull>(model_pull::Variables {
            device_id,
            model: input.model,
        })
        .await?
        .pull_model;
    Ok(json!({ "copy": json(&copy) }))
}

async fn remove(hub: &Hub, arguments: Option<JsonObject>) -> Result<Value, ToolError> {
    let input: CopyInput = parse(arguments)?;
    let device_id = resolve(hub, &input.device).await?;
    let found = fetch(hub)
        .await?
        .into_iter()
        .find(|copy| copy.place.id == device_id && copy.address == input.model);
    let Some(copy) = found else {
        return Ok(json!({ "copy": null }));
    };
    let left = hub
        .run::<ModelCopyRemove>(model_copy_remove::Variables { id: copy.id })
        .await?
        .remove_model_copy;
    Ok(json!({ "copy": json(&left) }))
}
