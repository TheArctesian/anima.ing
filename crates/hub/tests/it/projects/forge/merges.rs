//! Merging pull requests through GraphQL against the loopback GitHub (OD-E14, PRJ-028, PRJ-033,
//! PRO-013; K-E1 Section 13.3), and the text that never reaches a log, an error or a record.

mod bulk;
mod forks;
mod made;
mod outcomes;
mod refusals;
mod secrets;
mod tokens;

use axum::{
    body::Body,
    http::{Request, header},
};
use serde_json::{Value, json};

use super::{BASE, Forge, NAME, ok, pull, sha};
use crate::sources::stub::Heard;

pub const MERGE: &str = "mutation($input: MergePullRequestInput!) { mergePullRequest(input: \
    $input) { repository number state headSha mergedAt fromFork mergeability { state canMerge } } }";

pub const MERGE_MANY: &str = "mutation($pullRequests: [PullRequestTargetInput!]!, \
    $method: MergeMethod!) { mergePullRequests(pullRequests: $pullRequests, method: $method) { \
    repository number outcome message mergeSha } }";

pub const LIST: &str =
    "query($repository: ID!) { pullRequests(repository: $repository) { edges { cursor } } }";

pub const FORK: &str = "stranger/widgets";

pub fn merge_sha() -> String {
    sha(0xbeef)
}

pub fn open(number: u64, state: &str, mergeable: Value) -> Value {
    let mut one = pull(number, "open");
    one["mergeable_state"] = json!(state);
    one["mergeable"] = mergeable;
    one["comments"] = json!(0);
    one
}

pub fn clean(number: u64) -> Value {
    open(number, "clean", json!(true))
}

pub fn forked(number: u64) -> Value {
    let mut one = clean(number);
    one["head"]["repo"] = json!({ "id": 2, "full_name": FORK });
    one
}

pub fn merged(number: u64) -> Value {
    let mut one = pull(number, "closed");
    one["merged"] = json!(true);
    one["merged_at"] = json!("2030-05-03T10:00:00Z");
    one["comments"] = json!(0);
    one
}

pub fn methods(forge: &Forge) {
    forge.ok(
        "",
        &json!({ "id": 1, "full_name": NAME, "allow_merge_commit": true,
            "allow_squash_merge": true, "allow_rebase_merge": false }),
    );
}

pub fn reads(forge: &Forge, number: u64, before: &Value, after: &Value) {
    let path = format!("{BASE}/pulls/{number}");
    forge
        .kit
        .stub
        .reply_once("GET", &path, ok(&before.to_string()));
    forge.ok(&format!("/pulls/{number}"), after);
}

pub fn merges(forge: &Forge, number: u64) {
    let made = json!({ "sha": merge_sha(), "merged": true, "message": "Pull Request merged" });
    super::changes::answer(
        forge,
        ("PUT", &format!("/pulls/{number}/merge")),
        200,
        &made,
    );
}

pub fn script(forge: &Forge, number: u64) {
    methods(forge);
    reads(forge, number, &clean(number), &merged(number));
    merges(forge, number);
}

pub fn input(forge: &Forge, number: u64, extra: &Value) -> Value {
    let mut input = json!({ "repository": forge.repository, "number": number,
        "expectedHeadSha": sha(number), "method": "SQUASH" });
    if let (Value::Object(input), Value::Object(extra)) = (&mut input, extra) {
        input.extend(extra.clone());
    }
    json!({ "input": input })
}

pub async fn merge(forge: &Forge, number: u64, extra: &Value) -> Value {
    forge.kit.post(MERGE, input(forge, number, extra)).await
}

pub fn sent(forge: &Forge, number: u64) -> Vec<Heard> {
    super::changes::sent(forge, "PUT", &format!("/pulls/{number}/merge"))
}

pub fn changes_sent(forge: &Forge) -> usize {
    forge.kit.stub.with(|s| {
        s.heard
            .iter()
            .filter(|h| h.method != "GET" && h.path.starts_with(BASE))
            .count()
    })
}

/// Posts `query` with `token` and the agent declaration (LOG-016).
pub async fn as_agent(forge: &Forge, token: &str, query: &str, variables: Value) -> Value {
    let body = json!({ "query": query, "variables": variables }).to_string();
    let request = Request::post("/graphql")
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .header("anima-actor", "agent")
        .body(Body::from(body))
        .unwrap();
    let (status, text) = forge.kit.hub.send(request).await;
    assert!(status.is_success(), "{status} {text}");
    serde_json::from_str(&text).unwrap()
}
