//! A shortened answer of the server: stored first, marked, whole after a fault, and read back
//! in parts by the tool (SHA-004, SHA-006, SHA-016, SHA-017).

use std::time::SystemTime;

use serde_json::{Map, Value, json};

use super::store::Store;
use super::{FOLDER, Shortening, measure, original, setting};
use crate::mcp::outcome::ToolError;

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
    Shortening::from_setting(config, false)
}

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
    let default = Shortening::from_setting(config.path(), false);
    assert_eq!(default.applied("t", long_answer(), now), long_answer());
    assert!(
        !config.path().join(FOLDER).exists(),
        "nothing is written while it is off"
    );
    let never = {
        on(config.path());
        Shortening::from_setting(config.path(), true)
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
    let measured = measure::read(store.dir());
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
    let store = Store::new(config.path().join(FOLDER));
    let now = SystemTime::now();
    for handle in ["0".repeat(24), "../../state".to_owned()] {
        let refused = original::call(&store, arguments(json!({ "handle": handle })), now);
        let Err(ToolError::Refused(sentence)) = refused else {
            panic!("{refused:?}");
        };
        assert!(sentence.contains("Call the first tool again"), "{sentence}");
    }
    let handle = store
        .put("t", &json!({ "a": [1, 2], "b": { "c": [1, 2, 3] } }), now)
        .expect("stored");
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
