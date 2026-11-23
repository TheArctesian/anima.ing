//! A shortened answer of the server: stored first, marked, whole after a fault, and read back
//! in parts by the tool (SHA-004, SHA-006, SHA-016, SHA-017).

use std::time::SystemTime;

use serde_json::{Map, Value, json};

use super::store::{Original, Store};
use super::{FOLDER, Shortening, measure, original, originals_dir, setting};
use crate::mcp::outcome::ToolError;
use crate::mcp::untrusted::RESULT_CAP;

fn long_answer() -> Map<String, Value> {
    let events: Vec<Value> = (0..300)
        .map(|row| {
            let message = if row == 123 {
                "disk is full: write failed".to_owned()
            } else {
                format!("check ping of nas-{} passed", row % 9)
            };
            json!({ "id": format!("ev-{row:04}"), "severity": "INFO", "message": message })
        })
        .collect();
    match json!({ "events": events, "nextCursor": "c-300" }) {
        Value::Object(fields) => fields,
        _ => unreachable!(),
    }
}

fn on(config: &std::path::Path) -> Shortening {
    setting::write(&config.join(FOLDER), setting::State::On).expect("saved");
    Shortening::from_setting(config, HUB, false)
}

const HUB: &str = "https://hub.example";

fn arguments(value: Value) -> Option<Map<String, Value>> {
    match value {
        Value::Object(fields) => Some(fields),
        _ => None,
    }
}

#[test]
fn sha_017_nothing_is_shortened_until_it_is_turned_on_and_the_flag_wins() {
    let config = tempfile::tempdir().expect("a folder");
    let now = SystemTime::now();
    let default = Shortening::from_setting(config.path(), HUB, false);
    assert_eq!(default.applied("t", long_answer(), now), long_answer());
    assert!(
        !config.path().join(FOLDER).exists(),
        "nothing is written while it is off"
    );
    let never = {
        on(config.path());
        Shortening::from_setting(config.path(), HUB, true)
    };
    assert_eq!(never.applied("t", long_answer(), now), long_answer());
    assert!(never.store().is_none());
}

#[test]
fn sha_004_a_shortened_answer_names_a_handle_that_gives_each_row_back() {
    let config = tempfile::tempdir().expect("a folder");
    let shortening = on(config.path());
    let now = SystemTime::now();
    let whole = long_answer();
    let short = shortening.applied("anima_events_search", whole.clone(), now);
    let shown = short["events"].as_array().expect("a list");
    assert!(shown.len() < 30, "{}", shown.len());
    assert!(
        shown.contains(&whole["events"][123]),
        "the row that reports a fault stays"
    );
    let mark = &short["shortened"];
    assert_eq!(mark["tool"], original::TOOL);
    let store = shortening.store().expect("on");
    let mut rows = Vec::new();
    let mut from = Some(0_u64);
    while let Some(place) = from {
        let part = original::call(
            store,
            arguments(json!({ "handle": mark["handle"], "from": place })),
            now,
        )
        .expect("a part");
        assert_eq!(part["tool"], "anima_events_search");
        assert_eq!(part["list"], "events");
        assert_eq!(part["totalRows"], 300);
        rows.extend(part["rows"].as_array().expect("rows").iter().cloned());
        from = part["nextFrom"].as_u64();
    }
    assert_eq!(Value::Array(rows), whole["events"]);
    let measured = measure::read(&config.path().join(FOLDER));
    let sizes = measured["anima_events_search"];
    assert_eq!(sizes.answers, 1);
    assert!(sizes.bytes_after * 2 < sizes.bytes_before, "{sizes:?}");
}

#[test]
fn sha_006_an_answer_whose_original_cannot_be_stored_stays_whole() {
    let config = tempfile::tempdir().expect("a folder");
    let shortening = on(config.path());
    let folder = config.path().join(FOLDER);
    std::fs::remove_dir_all(&folder).expect("removed");
    std::fs::write(&folder, "a file where the folder must be").expect("written");
    assert_eq!(
        shortening.applied("t", long_answer(), SystemTime::now()),
        long_answer()
    );
}

#[test]
fn sha_014_a_handle_that_is_gone_or_wrong_gives_a_sentence_and_no_row() {
    let config = tempfile::tempdir().expect("a folder");
    let store = Store::new(originals_dir(config.path(), HUB));
    let now = SystemTime::now();
    for handle in ["0".repeat(24), "../../state".to_owned()] {
        let refused = original::call(&store, arguments(json!({ "handle": handle })), now);
        let Err(ToolError::Refused(sentence)) = refused else {
            panic!("{refused:?}");
        };
        assert!(sentence.contains("Call the first tool again"), "{sentence}");
    }
    let stored = Original {
        tool: "t".to_owned(),
        answer: json!({ "a": [1, 2], "b": { "c": [1, 2, 3] } }),
        lists: Vec::new(),
    };
    let handle = store.put(&stored, now).expect("stored");
    let longest =
        original::call(&store, arguments(json!({ "handle": handle })), now).expect("a part");
    assert_eq!(
        (longest["list"].as_str(), longest["totalRows"].as_u64()),
        (Some("b.c"), Some(3))
    );
    let unknown = original::call(
        &store,
        arguments(json!({ "handle": handle, "list": "x" })),
        now,
    );
    assert!(matches!(unknown, Err(ToolError::Refused(sentence)) if sentence.contains("a, b.c")));
}

#[test]
fn sha_004_an_answer_whose_marked_form_does_not_fit_stays_whole_for_the_last_limit() {
    let config = tempfile::tempdir().expect("a folder");
    let shortening = on(config.path());
    let events: Vec<Value> = (0..2_400)
        .map(|row| {
            let message = if row < 400 {
                format!("timeout {}", "x".repeat(280))
            } else {
                "ok".to_owned()
            };
            json!({ "id": row, "message": message })
        })
        .collect();
    let Value::Object(whole) = json!({ "events": events }) else {
        unreachable!()
    };
    let given = shortening.applied("t", whole.clone(), SystemTime::now());
    assert_eq!(given, whole, "a mark must not be cut after it is written");
    assert!(Value::Object(given).to_string().len() > RESULT_CAP);
    let stored = shortening.store().expect("on").list().expect("listed");
    assert_eq!(stored, []);
}

#[test]
fn sha_016_the_tool_reads_the_list_that_was_cut_also_inside_a_row() {
    let config = tempfile::tempdir().expect("a folder");
    let store = Store::new(originals_dir(config.path(), HUB));
    let now = SystemTime::now();
    let lines: Vec<String> = (0..40).map(|line| format!("line {line}")).collect();
    let answer = json!({ "ids": (0..500).collect::<Vec<u32>>(), "builds": [{ "id": 1 }, { "id": 2, "lines": lines }] });
    let stored = Original {
        tool: "t".to_owned(),
        answer,
        lists: vec!["builds[1].lines".to_owned()],
    };
    let handle = store.put(&stored, now).expect("stored");
    let part = original::call(&store, arguments(json!({ "handle": handle })), now).expect("a part");
    assert_eq!(
        part["list"], "builds[1].lines",
        "the default is the list that lost rows, not the longest"
    );
    assert_eq!(part["rows"].as_array().map(Vec::len), Some(40));
    assert_eq!(part["nextFrom"], Value::Null);
    let other = original::call(
        &store,
        arguments(json!({ "handle": handle, "list": "ids", "from": 499 })),
        now,
    )
    .expect("a part");
    assert_eq!(other["rows"], json!([499]));
}

#[test]
fn sha_016_an_original_of_one_hub_is_not_read_in_a_run_for_a_different_hub() {
    let config = tempfile::tempdir().expect("a folder");
    let shortening = on(config.path());
    let now = SystemTime::now();
    let short = shortening.applied("t", long_answer(), now);
    let handle = short["shortened"]["handle"].clone();
    let other = Shortening::from_setting(config.path(), "https://other.example", false);
    let read = original::call(
        other.store().expect("on"),
        arguments(json!({ "handle": handle })),
        now,
    );
    assert!(matches!(read, Err(ToolError::Refused(_))));
}

#[test]
fn sha_016_each_list_that_the_mark_names_is_read_by_the_tool_also_inside_a_row() {
    let config = tempfile::tempdir().expect("a folder");
    let shortening = on(config.path());
    let now = SystemTime::now();
    let lines: Vec<String> = (0..200)
        .map(|line| format!("compiled unit {line} of the build"))
        .collect();
    let Value::Object(whole) =
        json!({ "builds": [{ "id": 1 }, { "id": 2, "log": { "lines": lines } }] })
    else {
        unreachable!()
    };
    let short = shortening.applied("t", whole.clone(), now);
    let mark = &short["shortened"];
    let named = mark["lists"][0]["list"].as_str().expect("a list");
    assert_eq!(named, "builds[1].log.lines");
    let store = shortening.store().expect("on");
    let part = original::call(
        store,
        arguments(json!({ "handle": mark["handle"], "list": named })),
        now,
    )
    .expect("a part");
    assert_eq!(part["rows"], whole["builds"][1]["log"]["lines"]);
}

#[test]
fn sha_012_the_old_originals_of_a_hub_that_is_not_used_go_at_the_next_start() {
    let config = tempfile::tempdir().expect("a folder");
    let shortening = on(config.path());
    let now = SystemTime::now();
    let short = shortening.applied("t", long_answer(), now);
    assert!(short.contains_key("shortened"));
    let store = shortening.store().expect("on");
    assert_eq!(store.list().expect("listed").len(), 1);
    super::remove_old_of_each_hub(config.path(), now + std::time::Duration::from_hours(25));
    assert_eq!(store.list().expect("listed"), []);
    assert_eq!(
        setting::read(&config.path().join(FOLDER)),
        setting::State::On,
        "only originals go"
    );
}
