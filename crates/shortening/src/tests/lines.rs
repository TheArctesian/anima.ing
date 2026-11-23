//! The lines of a log: what must stay, the level of a line and its shape (SHA-001, SHA-005).

use serde_json::json;

use crate::lines::{level, same};
use crate::{Limits, Reason, shortened};

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
