//! The two forms of a Codex rollout, from files made in the shapes that Codex 0.13x writes
//! (`captures/`; the text and the paths are made up).

use super::super::{EntryKind, Source, Thread, ThreadReader, ToolCall, ToolKind};

const ITEMS: &str = include_str!("captures/items.jsonl");
const EVENTS: &str = include_str!("captures/events.jsonl");

fn read(lines: &str) -> Thread {
    let mut reader = ThreadReader::new(Source::Codex);
    for line in lines.lines() {
        reader.feed(line.as_bytes());
    }
    reader.finish()
}

fn kinds(thread: &Thread) -> Vec<&'static str> {
    thread
        .entries
        .iter()
        .map(|entry| match entry.kind {
            EntryKind::Prompt { .. } => "prompt",
            EntryKind::Reply { .. } => "reply",
            EntryKind::Reasoning { .. } => "reasoning",
            EntryKind::Tool(_) => "tool",
            EntryKind::TurnEnd { .. } => "end",
            EntryKind::Compacted => "compacted",
        })
        .collect()
}

fn tools(thread: &Thread) -> Vec<&ToolCall> {
    thread
        .entries
        .iter()
        .filter_map(|entry| match &entry.kind {
            EntryKind::Tool(tool) => Some(tool),
            _ => None,
        })
        .collect()
}

/// A rollout of one script call of `exec` with `input`, ended with `output`.
fn script(input: &str, output: &str) -> String {
    let call = serde_json::json!({"type":"response_item","payload":{"type":"custom_tool_call","call_id":"c","name":"exec","input":input}});
    let out = serde_json::json!({"type":"response_item","payload":{"type":"custom_tool_call_output","call_id":"c","output":[{"type":"input_text","text":output}]}});
    format!("{call}\n{out}")
}

#[test]
fn thr_003_a_rollout_of_items_reads_as_a_thread_with_its_title() {
    let thread = read(ITEMS);
    assert_eq!(
        kinds(&thread),
        ["prompt", "reply", "tool", "tool", "reply", "end"]
    );
    assert_eq!(
        thread.title.as_deref(),
        Some("Create a file named codex.txt that holds the word hi, then reply with the single")
    );
    assert_eq!(thread.prompts, 1);
    assert!(matches!(
        thread.entries.first().map(|entry| &entry.kind),
        Some(EntryKind::Prompt { images: 1, .. })
    ));
    assert_eq!(thread.model.as_deref(), Some("gpt-5.5"));
    assert_eq!(thread.folder.as_deref(), Some("/work/project"));
}

#[test]
fn thr_005_a_script_of_exec_shows_the_command_it_runs_and_its_output() {
    let thread = read(ITEMS);
    let [command, change] = tools(&thread)[..] else {
        panic!("two tool calls");
    };
    assert_eq!(
        (command.kind, command.summary.as_str()),
        (ToolKind::Command, "pwd -P")
    );
    assert_eq!(
        (command.output.as_deref(), command.failed),
        (Some("/work/project\n"), false)
    );
    assert_eq!(
        (change.kind, change.summary.as_str()),
        (ToolKind::Edit, "/work/project/codex.txt")
    );
    assert!(
        change
            .patch
            .as_deref()
            .is_some_and(|patch| patch.contains("\n+hi"))
    );
    assert!(
        change
            .output
            .as_deref()
            .is_some_and(|output| output.starts_with("Success."))
    );
}

#[test]
fn thr_003_a_rollout_of_events_still_reads_as_before() {
    let thread = read(EVENTS);
    assert_eq!(
        kinds(&thread),
        ["prompt", "reasoning", "tool", "reply", "end"]
    );
    assert_eq!(thread.title.as_deref(), Some("Count the files here."));
    assert_eq!(thread.branch.as_deref(), Some("main"));
    assert_eq!(tools(&thread)[0].summary, "ls | wc -l");
}

#[test]
fn thr_005_a_script_that_failed_or_was_stopped_failed() {
    let ran = |output: &str| {
        let thread = read(&script(
            "text(await tools.exec_command({cmd:\"make\"}));",
            output,
        ));
        tools(&thread)[0].failed
    };
    assert!(ran("Script failed\nOutput:\nboom"));
    assert!(ran("aborted by user after 3s"));
    assert!(ran(
        "Script completed\nOutput:\n{\"exit_code\":2,\"output\":\"no rule\"}"
    ));
    assert!(!ran(
        "Script completed\nOutput:\n{\"exit_code\":0,\"output\":\"ok\"}"
    ));
}

#[test]
fn thr_005_a_command_item_with_no_script_is_its_own_call() {
    let line = r#"{"type":"event_msg","payload":{"type":"item_completed","item":{"type":"CommandExecution","id":"x","command":["/bin/zsh","-lc","false"],"status":"failed","aggregated_output":"","exit_code":1}}}"#;
    let thread = read(line);
    let call = tools(&thread)[0];
    assert_eq!((call.summary.as_str(), call.failed), ("false", true));
}

#[test]
fn thr_006_a_script_is_read_only_as_text() {
    // A command that is not a plain string is not read: the call keeps the name `exec`.
    let thread = read(&script(
        "tools.exec_command({cmd: `ls ${x}`})",
        "Script completed",
    ));
    assert_eq!(tools(&thread)[0].name, "exec");
    // JSON nested past the limit of the parser changes nothing.
    let deep = format!(
        r#"{{"type":"event_msg","payload":{{"type":"item_completed","item":{}1{}}}}}"#,
        "[".repeat(200),
        "]".repeat(200)
    );
    assert_eq!(read(&deep), Thread::default());
}

#[test]
fn thr_006_a_change_of_many_files_shows_a_known_number() {
    let changes: serde_json::Map<String, serde_json::Value> = (0..100)
        .map(|n| {
            (
                format!("/work/{n:03}"),
                serde_json::json!({"type":"add","content":"x"}),
            )
        })
        .collect();
    let line = serde_json::json!({"type":"event_msg","payload":{"type":"item_completed","item":{"type":"FileChange","id":"f","changes":changes,"status":"completed"}}});
    let thread = read(&line.to_string());
    let patch = tools(&thread)[0].patch.clone().unwrap_or_default();
    assert_eq!(patch.matches("*** Add File").count(), 64);
    assert!(patch.ends_with("[36 more files]"));
}

#[test]
fn thr_005_a_wait_for_a_script_is_a_command_and_a_wait_for_an_agent_is_not() {
    let wait = |arguments: &str| {
        let line = serde_json::json!({"type":"response_item","payload":{"type":"function_call","call_id":"w","name":"wait","arguments":arguments}});
        tools(&read(&line.to_string()))[0].kind
    };
    assert_eq!(
        wait(r#"{"cell_id":"7","yield_time_ms":1000}"#),
        ToolKind::Command
    );
    assert_eq!(wait(r#"{"ids":["a"]}"#), ToolKind::Agent);
}

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
        super::exec::first_tool(script).map(|(name, input)| (name, input["cmd"].clone()))
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
