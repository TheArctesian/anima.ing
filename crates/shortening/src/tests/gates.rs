//! The gates of an answer: which lists are cut at all (SHA-001, SHA-004, SHA-007).

use serde_json::{Value, json};

use crate::{Limits, shortened};

#[test]
fn sha_001_a_page_of_rows_with_an_identifier_of_its_own_is_still_shortened() {
    for count in [30, 50, 51, 100] {
        let rows: Vec<Value> = (0..count)
            .map(|row| json!({"id": format!("d-{row:03}"), "status": "ONLINE", "note": "x".repeat(120)}))
            .collect();
        let short = shortened(&json!({ "devices": rows }), &Limits::default());
        let kept = short.map(|short| short.cuts()[0].kept.len());
        assert_eq!(kept, Some(15), "{count} rows");
    }
}

#[test]
fn sha_007_at_most_eight_lists_lose_rows() {
    let list = |rows: usize| -> Vec<Value> {
        (0..rows)
            .map(|row| json!({"n": row, "note": "y".repeat(80)}))
            .collect()
    };
    let answer = |lists: usize| -> Value {
        Value::Object(
            (0..lists)
                .map(|each| (format!("list{each}"), Value::Array(list(60))))
                .collect(),
        )
    };
    assert_eq!(
        shortened(&answer(8), &Limits::default()).map(|short| short.cuts().len()),
        Some(8)
    );
    assert_eq!(shortened(&answer(9), &Limits::default()), None);
}

#[test]
fn sha_004_a_list_under_a_name_that_makes_its_path_unclear_is_not_cut() {
    let rows: Vec<Value> = (0..100)
        .map(|row| json!({"n": row, "note": "z".repeat(80)}))
        .collect();
    let whole = json!({ "a.b": rows, "c[0]": rows, "d": rows });
    let short = shortened(&whole, &Limits::default()).expect("shortened");
    let lists: Vec<&str> = short.cuts().iter().map(|cut| cut.list.as_str()).collect();
    assert_eq!(lists, ["d"]);
}
