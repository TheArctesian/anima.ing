use async_graphql::{Enum, ID, InputObject, Result, SimpleObject};
use serde::{Deserialize, Serialize, de::DeserializeOwned};

use super::refused;

macro_rules! tags {
    ($name:ident: $($variant:ident),+) => {
        #[expect(clippy::unused_async_trait_impl, reason = "the GraphQL enum derive generates async trait impls")]
        #[derive(Clone, Copy, Eq, PartialEq, Enum, Deserialize, Serialize)]
        #[graphql(rename_items = "snake_case")]
        #[serde(rename_all = "snake_case")]
        pub(super) enum $name { $($variant),+ }
    };
}

tags!(AgentThreadAction: Prompt, Answer, Stop);
tags!(AgentThreadRequestKind: Approval, Question);
tags!(AgentThreadStopKind: Interrupt, End);
tags!(AgentThreadDecision: AllowOnce, AllowSession, Decline);
tags!(AgentThreadValueKind: Choice, FreeText);
tags!(AgentThreadBinding: Waiting, Bound, Unavailable);
tags!(AgentThreadLaunchState: Queued, Claimed, Reconciling, Started, Refused, Failed, Stopped, Expired, Retired);
tags!(AgentThreadCommandState: Waiting, Delivered, Recorded, Refused, Expired, Retired);

#[derive(SimpleObject, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct AgentThreadLaunchReceipt {
    launch: ID,
    state: AgentThreadLaunchState,
    cause: Option<String>,
    thread: Option<ID>,
    binding: AgentThreadBinding,
    first_command: ID,
}

#[derive(SimpleObject, Deserialize)]
pub(super) struct AgentThreadCommandReceipt {
    command: ID,
    state: AgentThreadCommandState,
    cause: Option<String>,
}

#[derive(SimpleObject)]
pub(super) struct AgentThreadControlView {
    pub thread: ID,
    pub offer: Option<AgentThreadControlOffer>,
}

#[derive(SimpleObject, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct AgentThreadControlOffer {
    process: ID,
    revision: f64,
    actions: Vec<AgentThreadAction>,
    stop_targets: Vec<AgentThreadStopTarget>,
    requests: Vec<AgentThreadRequestOffer>,
}

#[derive(SimpleObject, Deserialize)]
struct AgentThreadStopTarget {
    kind: AgentThreadStopKind,
    process: ID,
    turn: Option<ID>,
}

#[derive(SimpleObject, Deserialize)]
struct AgentThreadRequestOffer {
    kind: AgentThreadRequestKind,
    id: ID,
    subject: String,
    details: Option<String>,
    decisions: Option<Vec<AgentThreadDecision>>,
    rules: Option<Vec<AgentThreadRule>>,
    questions: Option<Vec<AgentThreadQuestion>>,
}

#[derive(SimpleObject, Deserialize)]
struct AgentThreadRule {
    selector: ID,
    label: String,
}

#[derive(SimpleObject, Deserialize)]
struct AgentThreadChoice {
    id: ID,
    label: String,
}

#[derive(SimpleObject, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AgentThreadQuestion {
    key: String,
    text: String,
    choices: Vec<AgentThreadChoice>,
    many: bool,
    free_text: bool,
}

#[derive(InputObject, Serialize)]
pub(super) struct AgentThreadAnswerPayload {
    kind: AgentThreadRequestKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    decision: Option<AgentThreadDecision>,
    #[serde(skip_serializing_if = "Option::is_none")]
    selector: Option<ID>,
    #[serde(skip_serializing_if = "Option::is_none")]
    answers: Option<Vec<AgentThreadQuestionAnswer>>,
}

#[derive(InputObject, Serialize)]
struct AgentThreadQuestionAnswer {
    key: String,
    values: Vec<AgentThreadAnswerValue>,
}

#[derive(InputObject, Serialize)]
struct AgentThreadAnswerValue {
    kind: AgentThreadValueKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<ID>,
    #[serde(skip_serializing_if = "Option::is_none")]
    text: Option<String>,
}

#[derive(InputObject, Serialize)]
pub(super) struct AgentThreadStopTargetInput {
    kind: AgentThreadStopKind,
    process: ID,
    #[serde(skip_serializing_if = "Option::is_none")]
    turn: Option<ID>,
}

pub(super) fn input<T: DeserializeOwned>(from: &impl Serialize, code: &'static str) -> Result<T> {
    serde_json::to_value(from)
        .and_then(serde_json::from_value)
        .map_err(|_| refused(code))
}

pub(super) fn view<T: DeserializeOwned>(from: &impl Serialize) -> Result<T> {
    input(from, "INTERNAL")
}
