use std::time::Duration;

use uuid::Uuid;

use crate::model_work::cause::Cause;
use crate::model_work::order::{PullFile, PullSource};
use crate::model_work::pull::{
    MAX_PARTIAL, PART_BYTES, PARTIAL_AGE, PULL_CAUSES, PullOrder, address_segments, child_deadline,
    fits, partial_room, progress, pull_deadline, range_start, resume_from, stale,
};
use crate::model_work::releases::fetch::{Ending, reply_in, verdict_in};

fn source() -> PullSource {
    PullSource {
        organisation: "unsloth".to_owned(),
        repository: "Qwen3-8B-GGUF".to_owned(),
        revision: "a".repeat(40),
    }
}

fn file(name: &str) -> PullFile {
    PullFile {
        name: name.to_owned(),
        size_bytes: 1 << 30,
        sha256: "b".repeat(64),
    }
}

fn order() -> PullOrder {
    PullOrder {
        copy: Uuid::nil(),
        source: source(),
        format: "gguf".to_owned(),
        files: vec![file("Qwen3-8B-Q4_K_M.gguf")],
        models: std::env::temp_dir()
            .join("models")
            .to_string_lossy()
            .into_owned(),
        reserve: 0,
        parent: 1,
    }
}

#[test]
fn pull_004_a_whole_order_holds_and_counts_its_bytes() {
    assert_eq!(order().checked(), Ok(()));
    assert_eq!(order().total_bytes(), 1 << 30);
}

#[test]
fn pull_004_the_process_checks_each_member_again() {
    let changes: [fn(&mut PullOrder); 7] = [
        |order| order.format = "onnx".to_owned(),
        |order| order.source.revision = "main".to_owned(),
        |order| order.source.organisation = "..".to_owned(),
        |order| order.files[0].name = "../escape.gguf".to_owned(),
        |order| order.files[0].name = "model.py".to_owned(),
        |order| order.files.push(file("q/Qwen3-8B-Q4_K_M.gguf")),
        |order| order.models = "models".to_owned(),
    ];
    for change in changes {
        let mut order = order();
        change(&mut order);
        assert_eq!(order.checked(), Err(Cause::MemberUnknown), "{order:?}");
    }
}

#[test]
fn pull_004_the_order_reads_back_and_refuses_an_unknown_member() {
    let text = serde_json::to_string(&order()).expect("text");
    let back: PullOrder = serde_json::from_str(&text).expect("read");
    assert_eq!(back, order());
    let more = text.replacen('{', r#"{"token":"x","#, 1);
    assert!(serde_json::from_str::<PullOrder>(&more).is_err());
}

#[test]
fn sf_381_a_folder_of_a_file_is_its_own_segment() {
    let source = source();
    let segments = address_segments(&source, "q4/part-1.gguf");
    assert_eq!(
        segments,
        [
            "unsloth",
            "Qwen3-8B-GGUF",
            "resolve",
            &"a".repeat(40),
            "q4",
            "part-1.gguf"
        ]
    );
}

#[test]
fn pull_006_a_resume_starts_at_the_last_complete_part() {
    let size = 3 * PART_BYTES + 5;
    assert_eq!(resume_from(0, size), Some(0));
    assert_eq!(resume_from(PART_BYTES - 1, size), Some(0));
    assert_eq!(resume_from(2 * PART_BYTES + 7, size), Some(2 * PART_BYTES));
    assert_eq!(resume_from(size, size), None);
    assert_eq!(resume_from(size + 1, size), Some(0));
}

#[test]
fn pull_006_a_200_starts_the_file_again_and_a_206_must_start_where_asked() {
    let asked = 2 * PART_BYTES;
    let range = format!("bytes {asked}-99999999999/99999999999");
    assert_eq!(range_start(200, asked, None), Ok(0));
    assert_eq!(range_start(206, asked, Some(&range)), Ok(asked));
    assert_eq!(
        range_start(206, asked, Some("bytes 0-9/10")),
        Err(Cause::SourceFailed)
    );
    assert_eq!(range_start(206, asked, None), Err(Cause::SourceFailed));
    assert_eq!(
        range_start(206, 0, Some("bytes 0-9/10")),
        Err(Cause::SourceFailed)
    );
    assert_eq!(range_start(404, asked, None), Err(Cause::SourceFailed));
}

#[test]
fn pull_partial_copies_age_out_and_a_fifth_is_refused() {
    assert!(stale(PARTIAL_AGE));
    assert!(!stale(Duration::from_hours(7 * 24 - 1)));
    assert!(partial_room(MAX_PARTIAL - 1, false));
    assert!(!partial_room(MAX_PARTIAL, false));
    assert!(partial_room(MAX_PARTIAL, true));
}

#[test]
fn pull_008_the_bytes_still_to_download_must_fit_above_the_reserve() {
    assert!(fits(100, 100, 40, 40));
    assert!(!fits(99, 100, 40, 40));
    assert!(fits(10, 100, 200, 10));
}

#[test]
fn pull_007_progress_never_passes_the_total_and_gives_a_rate() {
    let report = progress(300, 100, 250, 10);
    assert_eq!(report.bytes, 250);
    assert_eq!(report.total_bytes, 250);
    assert_eq!(report.bytes_per_second, 15);
    assert_eq!(progress(50, 80, 100, 0).bytes_per_second, 0);
}

#[test]
fn a_pull_may_take_longer_for_more_bytes_up_to_a_day() {
    assert_eq!(pull_deadline(0), Duration::from_mins(10));
    assert!(pull_deadline(10 << 30) > pull_deadline(1 << 30));
    assert_eq!(pull_deadline(u64::MAX), Duration::from_hours(24));
    assert!(child_deadline(100 << 30) > pull_deadline(100 << 30) + Duration::from_mins(5));
}

#[test]
fn only_the_words_of_the_pull_process_pass_through() {
    for cause in PULL_CAUSES {
        assert_eq!(reply_in(Err(cause), &PULL_CAUSES), cause.as_str());
        assert_eq!(
            verdict_in(Ending::Exited(cause.as_str()), &PULL_CAUSES),
            Err(cause)
        );
    }
    assert_eq!(
        reply_in(Err(Cause::ArchiveRejected), &PULL_CAUSES),
        "fetch_failed"
    );
    assert_eq!(
        verdict_in(Ending::Exited("archive_rejected\n"), &PULL_CAUSES),
        Err(Cause::FetchFailed)
    );
    assert_eq!(verdict_in(Ending::Exited("done\n"), &PULL_CAUSES), Ok(()));
}
