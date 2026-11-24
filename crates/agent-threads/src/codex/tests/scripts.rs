//! The scripts of `exec`: a wait, a poll and the steps of Codex go to the call they belong to.

use super::{read, tools};

/// One line of a `response_item` of the type `kind` with the fields `body`.
fn response(kind: &str, body: &serde_json::Value) -> String {
    let mut payload = body.clone();
    payload["type"] = kind.into();
    serde_json::json!({"type":"response_item","payload":payload}).to_string()
}

fn exec_call(id: &str, script: &str) -> String {
    response(
        "custom_tool_call",
        &serde_json::json!({"call_id":id,"name":"exec","input":script}),
    )
}

fn exec_out(id: &str, kind: &str, parts: &[&str]) -> String {
    let parts: Vec<_> = parts
        .iter()
        .map(|text| serde_json::json!({"type":"input_text","text":text}))
        .collect();
    response(kind, &serde_json::json!({"call_id":id,"output":parts}))
}

#[test]
fn thr_005_a_wait_carries_the_rest_of_a_script_into_its_call() {
    let lines = [
        exec_call("e", "text(await tools.exec_command({cmd:\"make\"}));"),
        exec_out(
            "e",
            "custom_tool_call_output",
            &["Script running with cell ID 1"],
        ),
        response(
            "function_call",
            &serde_json::json!({"call_id":"w","name":"wait","arguments":"{\"cell_id\":\"1\",\"yield_time_ms\":1000}"}),
        ),
        exec_out(
            "w",
            "function_call_output",
            &[
                "Script completed\nWall time 0.0 seconds\nOutput:\n",
                "{\"exit_code\":0,\"output\":\"built\\n\"}",
                "{\"exit_code\":128,\"output\":\"fatal: no repo\\n\"}",
            ],
        ),
    ];
    let thread = read(&lines.join("\n"));
    let calls = tools(&thread);
    assert_eq!(calls.len(), 1, "the wait is no row of its own");
    assert_eq!(calls[0].summary, "make");
    assert_eq!(
        (calls[0].output.as_deref(), calls[0].failed),
        (Some("built\nfatal: no repo\n"), true)
    );
    assert_eq!(thread.tail, crate::Tail::Open);
}

#[test]
fn thr_005_an_empty_poll_adds_to_the_command_it_reads() {
    let lines = [
        exec_call("e", "tools.exec_command({cmd:\"sleep 5; echo hi\"})"),
        exec_out(
            "e",
            "custom_tool_call_output",
            &[
                "Script completed\nOutput:\n",
                "{\"session_id\":99780,\"output\":\"\"}",
            ],
        ),
        exec_call(
            "p",
            "text(await tools.write_stdin({session_id:99780,chars:\"\",\"yield_time_ms\":60000}));",
        ),
        exec_out(
            "p",
            "custom_tool_call_output",
            &[
                "Script completed\nOutput:\n",
                "{\"exit_code\":0,\"output\":\"hi\\n\"}",
            ],
        ),
    ];
    let thread = read(&lines.join("\n"));
    let calls = tools(&thread);
    assert_eq!(calls.len(), 1, "the poll is no row of its own");
    assert_eq!(calls[0].output.as_deref(), Some("hi\n"));
}

#[test]
fn thr_005_a_script_left_open_does_not_take_the_steps_of_the_next_turn() {
    let command = r#"{"type":"event_msg","payload":{"type":"item_completed","item":{"type":"CommandExecution","id":"x","command":["ls"],"status":"completed","aggregated_output":"a","exit_code":0}}}"#;
    let turn = r#"{"type":"event_msg","payload":{"type":"task_started"}}"#;
    let prompt = r#"{"type":"event_msg","payload":{"type":"user_message","message":"next"}}"#;
    let lines = [
        exec_call("e", "tools.exec_command({cmd:\"ls\"})"),
        turn.to_owned(),
        prompt.to_owned(),
        command.to_owned(),
    ];
    let thread = read(&lines.join("\n"));
    let calls = tools(&thread);
    assert_eq!(calls.len(), 2);
    assert_eq!(calls[1].output.as_deref(), Some("a"));
}

#[test]
fn thr_005_the_first_tool_of_a_script_names_it_and_a_word_in_a_string_is_no_field() {
    let first = |script: &str| {
        super::super::exec::first_tool(script).map(|(name, input)| (name, input["cmd"].clone()))
    };
    assert_eq!(
        first("tools.apply_patch(\"x\");tools.exec_command({cmd:\"ls\"})").map(|(name, _)| name),
        Some("apply_patch")
    );
    assert_eq!(
        first("tools.exec_command({workdir:\"/x/cmd\", cmd:\"ls\"})"),
        Some(("exec_command", serde_json::json!("ls")))
    );
}

#[test]
fn thr_005_an_exec_function_call_with_json_reads_as_before() {
    let lines = [
        response(
            "function_call",
            &serde_json::json!({"call_id":"f","name":"exec","arguments":"{\"cmd\":\"ls\"}"}),
        ),
        response(
            "function_call_output",
            &serde_json::json!({"call_id":"f","output":"{\"output\":\"x\",\"metadata\":{\"exit_code\":1}}"}),
        ),
    ];
    let thread = read(&lines.join("\n"));
    let call = tools(&thread)[0];
    assert_eq!((call.summary.as_str(), call.failed), ("ls", true));
}

#[test]
fn thr_005_a_poll_that_writes_or_calls_more_is_a_call_of_its_own_and_a_poll_keeps_the_tool_open() {
    let first = [
        exec_call("e", "tools.exec_command({cmd:\"sleep 5\"})"),
        exec_out(
            "e",
            "custom_tool_call_output",
            &[
                "Script completed\nOutput:\n",
                "{\"session_id\":1,\"output\":\"\"}",
            ],
        ),
    ];
    let mut writes = first.to_vec();
    writes.push(exec_call("w", "await tools.write_stdin({session_id:1,chars:\"y\\n\"}); await tools.exec_command({cmd:\"rm -rf build\"})"));
    assert_eq!(tools(&read(&writes.join("\n"))).len(), 2);
    let mut polls = first.to_vec();
    polls.push(exec_call(
        "p",
        "tools.write_stdin({session_id:1,chars:\"\"})",
    ));
    assert_eq!(read(&polls.join("\n")).tail, crate::Tail::OpenTool);
}

#[test]
fn thr_005_a_script_shows_its_own_results_over_the_steps_of_another() {
    let step = r#"{"type":"event_msg","payload":{"type":"item_completed","item":{"type":"CommandExecution","id":"x","command":["make"],"status":"completed","aggregated_output":"built","exit_code":0}}}"#;
    let lines = [
        exec_call("b", "tools.exec_command({cmd:\"ls\"})"),
        step.to_owned(),
        exec_out(
            "b",
            "custom_tool_call_output",
            &[
                "Script completed\nOutput:\n",
                "{\"exit_code\":0,\"output\":\"a\"}",
            ],
        ),
    ];
    assert_eq!(
        tools(&read(&lines.join("\n")))[0].output.as_deref(),
        Some("a")
    );
}
