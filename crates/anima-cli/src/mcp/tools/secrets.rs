//! The operational-key tools of the `devices` set (K-E5 Section 2.2, OD-E23): two reads of the
//! metadata. No tool takes a value, makes a grant or ends a version (command line plan
//! Section 7): those are `anima secret` commands only.

use rmcp::model::JsonObject;
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::Value;

use crate::hub::Hub;
use crate::mcp::arguments::{json, object, parse};
use crate::mcp::catalogue::Access;
use crate::mcp::definition::{Definition, Effect};
use crate::mcp::outcome::ToolError;
use crate::mcp::untrusted::cleaned;
use crate::operational_keys::documents::{SecretList, SecretShow, secret_list, secret_show};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SecretTool {
    List,
    Show,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct NoInput {}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct NameInput {
    /// The name of the operational key.
    name: String,
}

impl SecretTool {
    pub const ALL: [Self; 2] = [Self::List, Self::Show];

    #[must_use]
    pub fn definition(self) -> Definition {
        match self {
            Self::List => Definition::new::<NoInput>(
                "anima_secrets_list",
                Access::Read,
                Effect::Reads,
                "Lists the operational keys of the operator's servers (API tokens, runtime \
                 secrets): name, provider, purpose, due state (CURRENT, DUE, EXPIRING, \
                 EXPIRED), the active version and the servers that may fetch each one. No \
                 field holds a value, and no tool gives one: only a server's daemon fetches \
                 a value. To add, rotate, revoke, remove or grant a key, tell the operator \
                 to run `anima secret`.",
            )
            .returns(&[("keys", "array", "The keys, with no value.")]),
            Self::Show => Definition::new::<NameInput>(
                "anima_secret_show",
                Access::Read,
                Effect::Reads,
                "Shows one operational key by name: its metadata, its versions with their \
                 states and times, and its consumers with the version that each daemon held \
                 at its last report. No field holds a value. `key` is null for an unknown \
                 name.",
            )
            .returns(&[("key", "object", "The key with no value, or null.")]),
        }
    }

    /// Runs the tool.
    ///
    /// # Errors
    /// [`ToolError`] for wrong arguments or the hub's refusal.
    pub async fn call(self, hub: &Hub, arguments: Option<JsonObject>) -> Result<Value, ToolError> {
        let result = match self {
            Self::List => {
                let NoInput {} = parse(arguments)?;
                let answer = hub.run::<SecretList>(secret_list::Variables).await?;
                object([("keys", cleaned(json(&answer.operational_keys)))])
            }
            Self::Show => {
                let NameInput { name } = parse(arguments)?;
                let variables = secret_show::Variables { name };
                let answer = hub.run::<SecretShow>(variables).await?;
                object([("key", cleaned(json(&answer.operational_key)))])
            }
        };
        Ok(Value::Object(result))
    }
}
