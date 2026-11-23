//! SEL-005, PULL-001, PULL-007, PULL-009, PULL-012, FIT-005, CMD-030; the server cleans and caps each string from
//! Hugging Face (MCP-008). A pull and a removal are open to an API key; the hub records the
//! actor (R-326).

use rmcp::model::JsonObject;
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::devices::get::resolve;
use crate::hub::Hub;
use crate::mcp::arguments::{json, object, parse, short};
use crate::mcp::catalogue::Access;
use crate::mcp::definition::{Definition, Effect};
use crate::mcp::outcome::ToolError;
use crate::model_copies::copies::fetch;
use crate::model_copies::documents::{ModelCopyRemove, ModelPull, model_copy_remove, model_pull};
use crate::model_copies::found;
use crate::model_fits;
use crate::model_library::documents::{ModelList, ModelShow, model_list, model_show};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModelTool {
    List,
    Show,
    Copies,
    Pull,
    Remove,
    Found,
    Find,
    Use,
    Fit,
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

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DeviceInput {
    /// The device's ID or its exact name.
    device: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UseInput {
    /// The device's ID or its exact name.
    device: String,
    /// The found model: its key, or the name that its tool shows.
    model: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FitInput {
    /// The model's name, an alias, or name:tag for one variant.
    model: String,
    /// Only this device: its ID or its exact name.
    device: Option<String>,
    /// The context in tokens; the default is 8192, or the context of the model when smaller.
    context: Option<u32>,
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
    pub const ALL: [Self; 9] = [
        Self::List,
        Self::Show,
        Self::Copies,
        Self::Pull,
        Self::Remove,
        Self::Found,
        Self::Find,
        Self::Use,
        Self::Fit,
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
            .returns(&[(
                "copy",
                "object",
                "What is left of the copy; null when gone.",
            )]),
            Self::Found | Self::Find | Self::Use => self.found_definition(),
            Self::Fit => fit_definition(),
        }
    }

    /// The tools of the found models (PULL-012, R-444).
    fn found_definition(self) -> Definition {
        match self {
            Self::Found => Definition::new::<ListInput>(
                "anima_model_found",
                Access::Read,
                Effect::Reads,
                "The models that each machine found where Ollama, LM Studio, the Hugging Face \
                 cache or a folder of its settings keeps them, from its last model report, with \
                 no copy that uses them: the machine, the tool, the name that the tool shows, \
                 the format, the size, whether it is usable (GGUF only) and its key. Never a \
                 path. Next: anima_model_find, anima_model_use.",
            )
            .returns(&[("found", "array", "The found models.")]),
            Self::Find => Definition::new::<DeviceInput>(
                "anima_model_find",
                Access::Write,
                Effect::Adds,
                "Asks one machine for its model state, so that it reads its found models again. \
                 Returns the job; when it is done, anima_model_found lists the models. A machine \
                 whose setting findModels is off lists none.",
            )
            .returns(&[("job", "string", "The ID of the job.")]),
            Self::Use => Definition::new::<UseInput>(
                "anima_model_use",
                Access::Write,
                Effect::Adds,
                "Uses a found model of one machine where it is, by its key or the name that its \
                 tool shows: the machine reads each file and keeps its size and SHA-256, checks \
                 them again at each load, and never writes in the folder of the tool. Only a \
                 GGUF model. Returns the copy; follow it with anima_model_copies.",
            )
            .returns(&[("copy", "object", "The copy that the hub asked for.")]),
            _ => unreachable!("only the tools of the found models"),
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
            Self::Found => found(hub, arguments).await,
            Self::Find => find(hub, arguments).await,
            Self::Use => use_found(hub, arguments).await,
            Self::Fit => fit(hub, arguments).await,
        }
    }
}

/// The tool of the fit of the variants on the machines (FIT-005, task L1-20).
fn fit_definition() -> Definition {
    Definition::new::<FitInput>(
        "anima_model_fit",
        Access::Read,
        Effect::Reads,
        "Whether each variant of a model (name:tag for one) fits each machine that sent \
         its hardware values, with the memory share of its device policy: state FITS, \
         REDUCED_CONTEXT (largestContext fits), TOO_LARGE or UNKNOWN (no layer shape); \
         placement GPU, PARTIAL (part in the system memory, slower) or CPU; the backend, \
         the context, the sizes in bytes and one sentence. An estimate, not a load. \
         Next: anima_model_pull.",
    )
    .returns(&[("fits", "array", "One fit for each variant on each machine.")])
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

async fn found(hub: &Hub, arguments: Option<JsonObject>) -> Result<Value, ToolError> {
    let _: ListInput = parse(arguments)?;
    Ok(json!({ "found": json(&found::fetch(hub).await?) }))
}

async fn find(hub: &Hub, arguments: Option<JsonObject>) -> Result<Value, ToolError> {
    let input: DeviceInput = parse(arguments)?;
    Ok(json!({ "job": found::find(hub, &input.device).await? }))
}

async fn use_found(hub: &Hub, arguments: Option<JsonObject>) -> Result<Value, ToolError> {
    let input: UseInput = parse(arguments)?;
    let copy = found::use_found(hub, &input.device, &input.model).await?;
    Ok(json!({ "copy": json(&copy) }))
}

async fn fit(hub: &Hub, arguments: Option<JsonObject>) -> Result<Value, ToolError> {
    let input: FitInput = parse(arguments)?;
    let asked = model_fits::Asked {
        model: input.model,
        device: input.device,
        context: input.context,
    };
    Ok(json!({ "fits": json(&model_fits::fetch(hub, asked).await?) }))
}
