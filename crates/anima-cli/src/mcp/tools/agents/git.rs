//! Read summaries of the live caller's worktree via the local runner (TC7 W3).

use agent_threads::git::Scope;
use agent_threads::git::read::{Outcome, Read, Refusal, Request};
use rmcp::model::JsonObject;
use schemars::JsonSchema;
use serde::Deserialize;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};

use crate::mcp::catalogue::Access;
use crate::mcp::definition::{Definition, Effect};
use crate::mcp::outcome::ToolError;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GitTool {
    Status,
    Checkpoints,
    Changes,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct StatusInput {
    /// The worktree repository name of the live local thread.
    repository: String,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct CheckpointsInput {
    /// The worktree repository name of the live local thread.
    repository: String,
    /// Page size, 1 through 100. Defaults to 25.
    limit: Option<u32>,
    /// The nextCursor from the previous page.
    after: Option<u32>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct ChangesInput {
    /// The worktree repository name of the live local thread.
    repository: String,
    /// Compare now, one turn, or the first and last checkpoints. Defaults to uncommitted.
    scope: Option<ChangeScope>,
    /// Required only for scope turn.
    turn: Option<u32>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
enum ChangeScope {
    Uncommitted,
    Turn,
    Thread,
}

const UNSUPPORTED: &str = "Git summaries require a supported Linux runner and holder.";
const UNAVAILABLE: &str = "The local runner could not return the Git summary. Tell the operator.";
const ARGUMENTS: &str =
    "The arguments do not fit the Git summary schema. Correct them and call again.";

impl GitTool {
    pub(super) fn definition(self) -> Definition {
        match self {
            Self::Status => Definition::new::<StatusInput>(
                "anima_code_git_status", Access::Read, Effect::Reads,
                "Summarize branch state and change counts for this live thread's local worktree. Requires a supported Linux runner.",
            ).returns(&[("status", "object", "Branch state flags, divergence and change counts.")]),
            Self::Checkpoints => Definition::new::<CheckpointsInput>(
                "anima_code_git_checkpoints", Access::Read, Effect::Reads,
                "List checkpoint turns and times for this live thread's local worktree. Requires a supported Linux runner.",
            ).returns(&[
                ("checkpoints", "array", "Turn numbers and times in seconds since the Unix epoch."),
                ("nextCursor", "integer", "The cursor for another page, when available."),
            ]),
            Self::Changes => Definition::new::<ChangesInput>(
                "anima_code_git_changes", Access::Read, Effect::Reads,
                "Summarize changed-file kinds and line counts for this live thread's local worktree. Patches and paths are omitted. Requires a supported Linux runner.",
            ).returns(&[("changes", "object", "Scope and change counts.")]),
        }
    }

    pub(super) async fn call(self, arguments: Option<JsonObject>) -> Result<Value, ToolError> {
        let request = self.request(arguments)?;
        match (self, read(request).await?) {
            (Self::Status, Outcome::Status { status }) => Ok(json!({ "status": status })),
            (
                Self::Checkpoints,
                Outcome::Checkpoints {
                    checkpoints,
                    next_cursor,
                },
            ) => {
                let mut value = json!({ "checkpoints": checkpoints });
                if let Some(cursor) = next_cursor {
                    value["nextCursor"] = json!(cursor);
                }
                Ok(value)
            }
            (Self::Changes, Outcome::Changes { changes }) => Ok(json!({ "changes": changes })),
            (_, Outcome::Refused { why }) => Err(refused(why)),
            _ => Err(ToolError::Refused(UNAVAILABLE.to_owned())),
        }
    }

    fn request(self, arguments: Option<JsonObject>) -> Result<Request, ToolError> {
        let (repository, read) = match self {
            Self::Status => (parse::<StatusInput>(arguments)?.repository, Read::Status),
            Self::Checkpoints => {
                let input: CheckpointsInput = parse(arguments)?;
                let limit = input.limit.unwrap_or(25);
                if !(1..=100).contains(&limit) {
                    return Err(ToolError::Refused(ARGUMENTS.to_owned()));
                }
                (
                    input.repository,
                    Read::Checkpoints {
                        limit,
                        after: input.after,
                    },
                )
            }
            Self::Changes => {
                let input: ChangesInput = parse(arguments)?;
                let scope = match (input.scope, input.turn) {
                    (None | Some(ChangeScope::Uncommitted), None) => Scope::Uncommitted,
                    (Some(ChangeScope::Thread), None) => Scope::Thread,
                    (Some(ChangeScope::Turn), Some(turn)) => Scope::Turn { turn },
                    _ => return Err(ToolError::Refused(ARGUMENTS.to_owned())),
                };
                (input.repository, Read::Changes { scope })
            }
        };
        if repository.is_empty() || repository.len() > 255 {
            return Err(ToolError::Refused(ARGUMENTS.to_owned()));
        }
        Ok(Request { repository, read })
    }
}

fn parse<T: DeserializeOwned>(arguments: Option<JsonObject>) -> Result<T, ToolError> {
    serde_json::from_value(Value::Object(arguments.unwrap_or_default()))
        .map_err(|_| ToolError::Refused(ARGUMENTS.to_owned()))
}

fn refused(why: Refusal) -> ToolError {
    let message = match why {
        Refusal::Unsupported => UNSUPPORTED,
        Refusal::Unbound => "This client could not be bound to a live local Anima thread.",
        Refusal::NoWorktree => "The live local thread has no registered worktree with that name.",
        Refusal::UnknownCheckpoint => "The requested checkpoint comparison is unavailable.",
        Refusal::Unavailable => UNAVAILABLE,
    };
    ToolError::Refused(message.to_owned())
}

#[cfg(all(target_os = "linux", target_pointer_width = "64"))]
async fn read(request: Request) -> Result<Outcome, ToolError> {
    let call = async {
        let link = crate::agents::runner::RunnerLink::connect().await.ok()?;
        link.git_read(request).await
    };
    tokio::time::timeout(std::time::Duration::from_secs(25), call)
        .await
        .ok()
        .flatten()
        .ok_or_else(|| ToolError::Refused(UNAVAILABLE.to_owned()))
}

#[cfg(not(all(target_os = "linux", target_pointer_width = "64")))]
#[expect(
    clippy::unused_async,
    reason = "the MCP call awaits the platform implementation"
)]
async fn read(_: Request) -> Result<Outcome, ToolError> {
    Err(refused(Refusal::Unsupported))
}
