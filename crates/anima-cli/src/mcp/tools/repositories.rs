//! The `repositories` tools (PRJ-028, PRJ-030 to PRJ-039). Merges, reruns, dispatches and
//! commit statuses have no tool: each is a trust decision (MCP-005).
//! Every text in a result was written by someone on GitHub: it goes in named fields, the server
//! cleans and cuts it (MCP-008), and the tool descriptions say that it is data.

mod code;
mod history;
mod issue_changes;
mod issues;
mod pull_requests;
mod workflows;

use rmcp::model::JsonObject;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};

use crate::failure::Failure;
use crate::forge::StateArg;
use crate::forge::comments::{IssueComments, issue_comments};
use crate::hub::Hub;
use crate::mcp::arguments::{Page, json, object};
use crate::mcp::definition::Definition;
use crate::mcp::outcome::ToolError;
use crate::projects::repository::resolve;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RepositoryTool {
    PullRequestsList,
    PullRequestGet,
    IssuesList,
    IssueGet,
    WorkflowsList,
    WorkflowRunsList,
    WorkflowRunGet,
    CommitsList,
    BranchesList,
    RepositoryReleasesList,
    CodeDirectoryList,
    IssueCreate,
    IssueCommentAdd,
    IssueSetOpen,
}

/// K1 `OpenOrClosed`.
#[derive(Clone, Copy, Debug, Default, Deserialize, JsonSchema)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum StateInput {
    #[default]
    Open,
    Closed,
    All,
}

impl From<StateInput> for StateArg {
    fn from(state: StateInput) -> Self {
        match state {
            StateInput::Open => Self::Open,
            StateInput::Closed => Self::Closed,
            StateInput::All => Self::All,
        }
    }
}

impl RepositoryTool {
    pub const ALL: [Self; 14] = [
        Self::PullRequestsList,
        Self::PullRequestGet,
        Self::IssuesList,
        Self::IssueGet,
        Self::WorkflowsList,
        Self::WorkflowRunsList,
        Self::WorkflowRunGet,
        Self::CommitsList,
        Self::BranchesList,
        Self::RepositoryReleasesList,
        Self::CodeDirectoryList,
        Self::IssueCreate,
        Self::IssueCommentAdd,
        Self::IssueSetOpen,
    ];

    #[must_use]
    pub fn definition(self) -> Definition {
        match self {
            Self::PullRequestsList => pull_requests::list_definition(),
            Self::PullRequestGet => pull_requests::get_definition(),
            Self::IssuesList => issues::list_definition(),
            Self::IssueGet => issues::get_definition(),
            Self::WorkflowsList => workflows::list_definition(),
            Self::WorkflowRunsList => workflows::runs_definition(),
            Self::WorkflowRunGet => workflows::run_definition(),
            Self::CommitsList => history::commits_definition(),
            Self::BranchesList => history::branches_definition(),
            Self::RepositoryReleasesList => history::releases_definition(),
            Self::CodeDirectoryList => code::directory_definition(),
            Self::IssueCreate => issue_changes::create_definition(),
            Self::IssueCommentAdd => issue_changes::comment_definition(),
            Self::IssueSetOpen => issue_changes::set_open_definition(),
        }
    }

    /// # Errors
    /// [`ToolError`] for wrong arguments, an unknown target, or the hub's refusal.
    #[expect(
        clippy::cognitive_complexity,
        reason = "a flat dispatch with one arm for each tool"
    )]
    pub async fn call(self, hub: &Hub, arguments: Option<JsonObject>) -> Result<Value, ToolError> {
        match self {
            Self::PullRequestsList => pull_requests::list(hub, arguments).await,
            Self::PullRequestGet => pull_requests::get(hub, arguments).await,
            Self::IssuesList => issues::list(hub, arguments).await,
            Self::IssueGet => issues::get(hub, arguments).await,
            Self::WorkflowsList => workflows::list(hub, arguments).await,
            Self::WorkflowRunsList => workflows::runs(hub, arguments).await,
            Self::WorkflowRunGet => workflows::run(hub, arguments).await,
            Self::CommitsList => history::commits(hub, arguments).await,
            Self::BranchesList => history::branches(hub, arguments).await,
            Self::RepositoryReleasesList => history::releases(hub, arguments).await,
            Self::CodeDirectoryList => code::directory(hub, arguments).await,
            Self::IssueCreate => issue_changes::create(hub, arguments).await,
            Self::IssueCommentAdd => issue_changes::comment(hub, arguments).await,
            Self::IssueSetOpen => issue_changes::set_open(hub, arguments).await,
        }
    }
}

/// The most a page of pull requests or runs may hold; the hub refuses more.
const SMALL_PAGE: i64 = 50;

async fn repository_id(hub: &Hub, name: &str) -> Result<String, ToolError> {
    resolve(hub, name).await.map_err(|failure| match failure {
        Failure::NotFound { message, .. } | Failure::Usage { message, .. } => ToolError::Refused(
            format!("{message} Call anima_projects_search for the repositories."),
        ),
        other => ToolError::Failed(other),
    })
}

fn size(page: &Page, largest: i64) -> i64 {
    page.size().min(largest)
}

fn listing(key: &str, connection: &impl Serialize) -> Value {
    let connection = json(connection);
    let rows: Vec<Value> = connection["edges"]
        .as_array()
        .map_or_else(Vec::new, |edges| {
            edges.iter().map(|edge| edge["node"].clone()).collect()
        });
    let more = connection["pageInfo"]["hasNextPage"] == json!(true);
    let next = if more {
        connection["pageInfo"]["endCursor"].clone()
    } else {
        Value::Null
    };
    let mut fields: Map<String, Value> = object([
        (key, Value::Array(rows)),
        ("nextCursor", next),
        ("read", connection["read"].clone()),
    ]);
    if let Some(methods) = connection.get("mergeMethods") {
        fields.insert("mergeMethods".to_owned(), methods.clone());
    }
    Value::Object(fields)
}

async fn with_comments(
    hub: &Hub,
    mut answer: Value,
    thread: (String, i64, Option<bool>),
    page: Page,
) -> Result<Value, ToolError> {
    let (repository, number, fresh) = thread;
    let variables = issue_comments::Variables {
        repository,
        number,
        first: Some(size(&page, 100)),
        after: page.after,
        fresh,
    };
    let comments = hub.run::<IssueComments>(variables).await?.issue_comments;
    let listed = listing("comments", &comments);
    answer["comments"] = listed["comments"].clone();
    answer["nextCursor"] = listed["nextCursor"].clone();
    Ok(answer)
}
