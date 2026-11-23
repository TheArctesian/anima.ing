//! The rules of a shortened answer on built lists (SHA-001 to SHA-007). The real answers of
//! Anima's tools are in the tests of `anima-cli` (SHA-023).

use serde_json::{Value, json};

use crate::lines::{level, same};
use crate::{FIELD, Limits, Reason, shortened};

/// A list of `count` events that are alike, with the rows of `odd` in place of some.
fn events(count: usize, odd: &[(usize, Value)]) -> Value {
    let rows: Vec<Value> = (0..count)
        .map(|row| {
            odd.iter()
                .find(|(place, _)| *place == row)
                .map_or_else(
                    || {
                        json!({
                            "id": format!("ev-{row:04}"),
                            "severity": "INFO",
                            "message": format!("check ping of nas-{} passed in {} ms", row % 7, 20 + row % 5),
                            "latencyMs": 20 + row % 5,
                        })
                    },
                    |(_, value)| value.clone(),
                )
        })
        .collect();
    json!({ "events": rows, "nextCursor": "c-200" })
}

fn rows_of(answer: &Value) -> &Vec<Value> {
    answer["events"].as_array().expect("a list")
}

#[test]
fn sha_001_a_row_that_reports_a_fault_or_is_not_like_the_others_stays() {
    let odd = [
        (
            57,
            json!({"id": "ev-0057", "severity": "ERROR", "message": "disk is full", "latencyMs": 22}),
        ),
        (
            90,
            json!({"id": "ev-0090", "severity": "INFO", "message": "check ping passed", "latencyMs": 9000}),
        ),
        (
            120,
            json!({"id": "ev-0120", "severity": "INFO", "message": "ok", "latencyMs": 21, "rebootedBy": "ana"}),
        ),
        (
            150,
            json!({"id": "ev-0150", "severity": "INFO", "message": "ok", "latencyMs": 21, "lastError": "connection refused"}),
        ),
    ];
    let whole = events(200, &odd);
    let short = shortened(&whole, &Limits::default()).expect("a long list is shortened");
    let cut = &short.cuts()[0];
    assert_eq!((cut.list.as_str(), cut.rows), ("events", 200));
    assert_eq!(cut.kept.get(&57), Some(&Reason::Error));
    assert!(matches!(
        cut.kept.get(&90),
        Some(Reason::Rare | Reason::Far)
    ));
    assert_eq!(cut.kept.get(&120), Some(&Reason::Rare));
    assert_eq!(cut.kept.get(&150), Some(&Reason::Error));
    let answer = short.marked("0".repeat(24).as_str(), "anima_original_get");
    for (_, row) in &odd {
        assert!(rows_of(&answer).contains(row), "{row}");
    }
}

#[test]
fn sha_001_a_number_far_from_the_others_keeps_its_row() {
    let rows: Vec<Value> = (0..200_u32)
        .map(|row| json!({"name": format!("disk-{row}"), "usedBytes": if row == 77 { 9_000_000 } else { 1_000 + row * 13 }}))
        .collect();
    let short = shortened(&json!({ "disks": rows }), &Limits::default()).expect("shortened");
    assert_eq!(short.cuts()[0].kept.get(&77), Some(&Reason::Far));
    assert_eq!(short.cuts()[0].kept.len(), 15);
}

#[test]
fn sha_001_one_of_a_few_values_that_are_equally_frequent_is_not_rare() {
    let short = shortened(&events(200, &[]), &Limits::default()).expect("shortened");
    assert_eq!(short.cuts()[0].kept.len(), 15);
}

#[test]
fn sha_001_the_rows_that_must_stay_are_kept_above_the_limit() {
    let odd: Vec<(usize, Value)> = (0..60)
        .map(|row| (row * 3, json!({"id": format!("ev-{row}"), "severity": "INFO", "message": format!("backup {row} failed"), "latencyMs": 20})))
        .collect();
    let whole = events(200, &odd);
    let short = shortened(&whole, &Limits::default()).expect("shortened");
    let errors = short.cuts()[0]
        .kept
        .values()
        .filter(|reason| **reason == Reason::Error)
        .count();
    assert_eq!(errors, 60);
}

#[test]
fn sha_001_a_field_named_for_a_fault_that_holds_nothing_keeps_no_row() {
    let rows: Vec<Value> = (0..100)
        .map(|row| json!({"name": format!("nas-{row}"), "lastError": null, "failedChecks": 0, "note": "x".repeat(40)}))
        .collect();
    let short = shortened(&json!({ "devices": rows }), &Limits::default()).expect("shortened");
    assert_eq!(short.cuts()[0].kept.len(), 15);
}

#[test]
fn sha_003_a_kept_row_is_not_changed_and_the_order_stays() {
    let whole = events(
        200,
        &[(
            57,
            json!({"id": "x", "severity": "ERROR", "message": "m", "latencyMs": 1}),
        )],
    );
    let short = shortened(&whole, &Limits::default()).expect("shortened");
    let places: Vec<usize> = short.cuts()[0].kept.keys().copied().collect();
    let answer = short.marked("h", "t");
    let kept: Vec<&Value> = places
        .iter()
        .map(|place| &rows_of(&whole)[*place])
        .collect();
    assert_eq!(rows_of(&answer).iter().collect::<Vec<_>>(), kept);
    assert_eq!(answer["nextCursor"], whole["nextCursor"]);
    let fields: Vec<&String> = answer.as_object().expect("an object").keys().collect();
    assert_eq!(fields, ["events", "nextCursor", FIELD]);
}

#[test]
fn sha_004_the_mark_says_what_was_left_out_and_how_to_get_it() {
    let short = shortened(&events(200, &[]), &Limits::default()).expect("shortened");
    let answer = short.marked("abc123", "anima_original_get");
    let mark = &answer[FIELD];
    assert_eq!(mark["handle"], "abc123");
    assert_eq!(mark["tool"], "anima_original_get");
    let list = &mark["lists"][0];
    assert_eq!(
        (list["list"].as_str(), list["rows"].as_u64()),
        (Some("events"), Some(200))
    );
    let kept = list["kept"].as_u64().expect("a number");
    assert_eq!(list["leftOut"].as_u64(), Some(200 - kept));
    assert_eq!(rows_of(&answer).len() as u64, kept);
    assert_eq!(list["keptFor"]["first"], 4);
    assert_eq!(list["keptFor"]["last"], 2);
    let note = mark["note"].as_str().expect("a text");
    assert!(
        note.contains("NOT the whole answer") && note.contains("anima_original_get"),
        "{note}"
    );
}

#[test]
fn sha_005_the_same_answer_gives_the_same_bytes() {
    let whole = events(
        300,
        &[(
            7,
            json!({"id": "q", "severity": "WARNING", "message": "slow", "latencyMs": 400}),
        )],
    );
    let once = shortened(&whole, &Limits::default())
        .expect("shortened")
        .marked("h", "t");
    let twice = shortened(&whole, &Limits::default())
        .expect("shortened")
        .marked("h", "t");
    assert_eq!(once.to_string(), twice.to_string());
}

#[test]
fn sha_007_a_small_answer_a_short_list_and_a_small_saving_stay_as_they_are() {
    let limits = Limits::default();
    assert_eq!(shortened(&events(4, &[]), &limits), None);
    assert_eq!(shortened(&events(15, &[]), &limits), None);
    assert_eq!(shortened(&json!([1, 2, 3]), &limits), None);
    let all_errors: Vec<(usize, Value)> = (0..200)
        .map(|row| (row, json!({"id": row, "message": "fatal"})))
        .collect();
    assert_eq!(shortened(&events(200, &all_errors), &limits), None);
    let mut marked = events(200, &[]);
    marked[FIELD] = json!(true);
    assert_eq!(shortened(&marked, &limits), None);
}

#[test]
fn sha_003_a_list_inside_an_object_is_found_and_a_list_of_numbers_is_not_touched() {
    let inner = events(120, &[]);
    let whole = json!({ "page": { "entries": inner["events"] }, "samples": (0..500).collect::<Vec<u32>>() });
    let short = shortened(&whole, &Limits::default()).expect("shortened");
    assert_eq!(short.cuts().len(), 1);
    assert_eq!(short.cuts()[0].list, "page.entries");
    assert_eq!(short.marked("h", "t")["samples"], whole["samples"]);
}

fn log() -> Vec<String> {
    let mut lines: Vec<String> = (0..300)
        .map(|line| {
            format!(
                "2026-11-22T10:{:02}:{:02}Z INFO compiled unit {line} of 300",
                line / 60,
                line % 60
            )
        })
        .collect();
    lines[100] = "2026-11-22T10:01:40Z ERROR build step 4 broke".to_owned();
    lines[101] = "    at compile (/srv/build/step.rs:44)".to_owned();
    lines[102] = "    at main (/srv/build/main.rs:9)".to_owned();
    lines[180] = "WARN: cache miss for layer 12".to_owned();
    lines[181] = "WARN: cache miss for layer 13".to_owned();
    lines[250] = "the upload was rejected by the registry".to_owned();
    lines
}

#[test]
fn sha_001_each_error_line_its_trace_and_the_first_warning_of_a_shape_stay() {
    let whole = json!({ "lines": log() });
    let short = shortened(&whole, &Limits::default()).expect("a long log is shortened");
    let kept = &short.cuts()[0].kept;
    assert_eq!(kept.get(&100), Some(&Reason::Error));
    assert_eq!(kept.get(&101), Some(&Reason::Trace));
    assert_eq!(kept.get(&102), Some(&Reason::Trace));
    assert_eq!(kept.get(&103), None);
    assert_eq!(kept.get(&180), Some(&Reason::Warning));
    assert_eq!(kept.get(&181), None);
    assert_eq!(kept.get(&250), Some(&Reason::Error));
    assert!(kept.len() <= 15 + 5, "{}", kept.len());
}

#[test]
fn sha_001_a_level_is_a_whole_word_in_any_case() {
    use level::{Level, level};
    assert_eq!(level("12:00 [Error] no route"), Level::Error);
    assert_eq!(level("tests: 3 FAILED"), Level::Error);
    assert_eq!(level("warning: unused import"), Level::Warning);
    assert_eq!(level("0 errors, error_count=0"), Level::Other);
    assert_eq!(level("INFO started"), Level::Other);
}

#[test]
fn sha_005_lines_with_different_numbers_have_one_shape_and_different_messages_do_not() {
    use same::shape;
    assert_eq!(
        shape("WARN: retry 3 of 5 at 0x7ffe12 in /var/lib/anima/run.log"),
        shape("WARN: retry 4 of 5 at 0x1a in /tmp/x/run.log")
    );
    assert_eq!(shape("WARN: retry 3 at 0x7f"), "WARN: retry N at ADDR");
    assert_ne!(shape("disk 3: full"), shape("disk 4: full"));
    assert_ne!(shape("WARN: disk full"), shape("WARN: link down"));
    assert_eq!(shape("höhe: 12 µm"), "höhe: N µm");
}

/// A small generator with a fixed seed: the same lists in each run.
struct Dice(u64);

impl Dice {
    fn roll(&mut self, sides: u64) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        (self.0 >> 33) % sides
    }
}

#[test]
fn sha_001_sha_003_on_many_built_lists_no_fault_is_lost_and_no_row_is_changed() {
    let mut dice = Dice(22);
    for _ in 0..200 {
        let count = usize::try_from(16 + dice.roll(400)).expect("small");
        let rows: Vec<Value> = (0..count)
            .map(|row| match dice.roll(40) {
                0 => json!({"id": row, "state": "UP", "note": "the job crashed", "ms": dice.roll(50)}),
                1 => json!({"id": row, "state": "DEGRADED", "note": "fine", "ms": dice.roll(50)}),
                2 => json!({"id": row, "state": "UP", "note": "fine", "ms": 90_000 + dice.roll(50)}),
                3 => json!({"id": row, "state": "UP", "note": "fine", "ms": dice.roll(50), "extra": true}),
                _ => json!({"id": row, "state": "UP", "note": "fine", "ms": dice.roll(50)}),
            })
            .collect();
        let whole = json!({ "rows": rows });
        let Some(short) = shortened(&whole, &Limits::default()) else {
            continue;
        };
        let answer = short.marked("h", "t");
        let kept = answer["rows"].as_array().expect("a list");
        let mut next = kept.iter().peekable();
        for row in &rows {
            let stays = next.peek().is_some_and(|kept| *kept == row);
            if stays {
                next.next();
            }
            let odd = row["note"] != "fine"
                || row["state"] != "UP"
                || row.get("extra").is_some()
                || row["ms"].as_u64() >= Some(90_000);
            assert!(stays || !odd, "lost {row}");
        }
        assert_eq!(
            next.next(),
            None,
            "a kept row is not a row of the original, or the order changed"
        );
    }
}
