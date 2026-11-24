//! The MCP tools of a model in the memory of the machines: its fit (FIT-005, task L1-20) and its keep (FIT-022,
//! FIT-023, task L1-16).

use rmcp::model::JsonObject;
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::hub::Hub;
use crate::mcp::arguments::{json, parse};
use crate::mcp::catalogue::Access;
use crate::mcp::definition::{Definition, Effect};
use crate::mcp::outcome::ToolError;
use crate::model_fits;

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct FitInput {
    /// The model's name, an alias, or name:tag for one variant.
    model: String,
    /// Only this device: its ID or its exact name.
    device: Option<String>,
    /// The context in tokens; the default is 8192, or the context of the model when smaller.
    context: Option<u32>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct KeepInput {
    /// The device's ID or its exact name.
    device: String,
    /// The variant, name:tag, such as qwen3:8b-q4_k_m.
    model: String,
    /// A pinned copy stays loaded, and comes back when other work took its memory.
    pinned: bool,
    /// Seconds with no use before the machine unloads the copy, 60 to 86400; 900 when not given.
    idle_seconds: Option<u32>,
}

/// The tool of the fit of the variants on the machines (FIT-005, task L1-20).
pub(super) fn fit_definition() -> Definition {
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

/// The tool of the keep of a copy (L1-16, FIT-022, FIT-023).
pub(super) fn keep_definition() -> Definition {
    Definition::new::<KeepInput>(
        "anima_model_keep",
        Access::Write,
        Effect::Sets,
        "Sets whether one machine keeps its copy of a variant name:tag loaded: pinned, it \
         stays loaded and comes back when other work took its memory; else the machine \
         unloads it after idleSeconds with no use. Only a copy on its machine \
         (DOWNLOADED or VALIDATED). Returns the job; anima_model_copies shows the load.",
    )
    .returns(&[("job", "string", "The ID of the job.")])
}

pub(super) async fn fit(hub: &Hub, arguments: Option<JsonObject>) -> Result<Value, ToolError> {
    let input: FitInput = parse(arguments)?;
    let asked = model_fits::Asked {
        model: input.model,
        device: input.device,
        context: input.context,
    };
    Ok(json!({ "fits": json(&model_fits::fetch(hub, asked).await?) }))
}

pub(super) async fn keep(hub: &Hub, arguments: Option<JsonObject>) -> Result<Value, ToolError> {
    let input: KeepInput = parse(arguments)?;
    let asked = crate::model_copies::keep::Keep {
        device: &input.device,
        model: &input.model,
        pinned: input.pinned,
        idle_seconds: input.idle_seconds,
    };
    Ok(json!({ "job": crate::model_copies::keep::keep(hub, &asked).await? }))
}
