use serde_json::json;

use crate::mcp::session::{Desk, LOGIN_TOKEN, hub};

const STATUS: &str = "anima_code_git_status";
const CHECKPOINTS: &str = "anima_code_git_checkpoints";
const CHANGES: &str = "anima_code_git_changes";
#[cfg(all(target_os = "linux", target_pointer_width = "64"))]
const SENTINEL: &str = "fixture-sentinel-for-git-read";

#[tokio::test]
async fn tc7_w3_only_summary_tools_are_listed_and_unknown_selectors_are_refused() {
    let hub = hub().await;
    let (mut session, _) = Desk::new()
        .start(&hub, LOGIN_TOKEN, &["--toolsets", "agents"])
        .await;
    let tools = session.request("tools/list", json!({})).await;
    let listed: Vec<_> = tools["tools"]
        .as_array()
        .expect("tools")
        .iter()
        .filter(|tool| {
            tool["name"]
                .as_str()
                .is_some_and(|name| name.starts_with("anima_code_git_"))
        })
        .collect();
    let mut names: Vec<_> = listed
        .iter()
        .filter_map(|tool| tool["name"].as_str())
        .collect();
    names.sort_unstable();
    assert_eq!(names, [CHANGES, CHECKPOINTS, STATUS]);
    assert!(
        listed
            .iter()
            .all(|tool| tool["annotations"]["readOnlyHint"] == true)
    );
    let rejected = session
        .call(
            STATUS,
            json!({"repository":"work","thread":"fixture-rejected-selector"}),
        )
        .await;
    assert_eq!(rejected["isError"], true);
    assert!(!rejected.to_string().contains("fixture-rejected-selector"));
    session.finish().await;
}

#[cfg(not(all(target_os = "linux", target_pointer_width = "64")))]
#[tokio::test]
async fn tc7_w3_unsupported_platform_has_a_fixed_tool_refusal() {
    let hub = hub().await;
    let (mut session, _) = Desk::new().start(&hub, LOGIN_TOKEN, &[]).await;
    let answer = session.call(STATUS, json!({"repository":"work"})).await;
    assert_eq!(answer["isError"], true);
    assert_eq!(
        answer["content"][0]["text"],
        "Git summaries require a supported Linux runner and holder."
    );
    session.finish().await;
}

#[cfg(all(target_os = "linux", target_pointer_width = "64"))]
mod runner;
