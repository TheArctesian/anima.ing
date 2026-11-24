use crate::drive::request::visible;
use crate::drive::{Answer, DriveError, ProgramRequestId, Request, RequestKind};
use crate::{MAX_OUTPUT, held};
use serde_json::{Value, json};

pub(super) fn request(value: &Value) -> Option<Request> {
    let id = ProgramRequestId::from_json(&value["id"])?;
    let params = &value["params"];
    let options = params["options"].as_array()?;
    if options.len() > 32 {
        return None;
    }
    let offered: Vec<Value> = options
        .iter()
        .filter(|option| {
            matches!(
                option["kind"].as_str(),
                Some("allow_once" | "allow_always" | "reject_once" | "reject_always")
            ) && option["optionId"]
                .as_str()
                .is_some_and(|id| !id.is_empty() && id.len() <= 1024)
        })
        .cloned()
        .collect();
    let ids: std::collections::BTreeSet<_> = offered
        .iter()
        .filter_map(|option| option["optionId"].as_str())
        .collect();
    if offered.len() != options.len() || ids.len() != offered.len() {
        return None;
    }
    let tool = &params["toolCall"];
    let kind = match tool["kind"].as_str() {
        Some("execute") => RequestKind::Command,
        Some("edit" | "delete" | "move") => RequestKind::FileChange,
        _ => RequestKind::Tool,
    };
    // ACP's allow_always has provider-defined persistence, not necessarily session scope.
    // Only allow_once is exposed as Allow; never broaden it to a persistent grant.
    Some(Request {
        id,
        kind,
        tool: visible(&held(tool["title"].as_str().unwrap_or("Grok tool"), 1024)),
        subject: visible(&held(&tool["rawInput"].to_string(), MAX_OUTPUT)),
        diff: None,
        input: json!({"options":offered}),
        session_rules: Vec::new(),
        offered: Vec::new(),
        questions: Vec::new(),
    })
}

/// Answers only an option supplied by this request. Persistent grants are not session grants.
/// # Errors
/// Refuses an answer for which Grok offered no exact option.
pub fn answer(request: &Request, answer: &Answer) -> Result<String, DriveError> {
    answer.check(request)?;
    let kind = match answer {
        Answer::Allow => "allow_once",
        Answer::Deny { .. } => "reject_once",
        _ => {
            return Err(DriveError::AnswerDoesNotFit(
                "Grok takes a one-time allow or deny",
            ));
        }
    };
    let option = request.input["options"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|option| option["kind"] == kind)
        .and_then(|option| option["optionId"].as_str());
    let outcome = match (option, answer) {
        (Some(option), _) => json!({"outcome":"selected","optionId":option}),
        (None, Answer::Deny { .. }) => json!({"outcome":"cancelled"}),
        _ => {
            return Err(DriveError::AnswerDoesNotFit(
                "Grok offered no one-time allow",
            ));
        }
    };
    Ok(json!({"jsonrpc":"2.0","id":request.id.json(),"result":{"outcome":outcome}}).to_string())
}
