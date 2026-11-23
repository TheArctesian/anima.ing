use crate::model_work::drain::{DRAIN_LIMIT_SECONDS, Drain, Step};

#[test]
fn cap_013_a_drain_ends_in_60_seconds_or_fewer_with_requests_still_in_progress() {
    let drain = Drain::new(600, 1_000);
    let ended = (1_000..=2_000)
        .find(|now| drain.step(3, *now) != Step::Wait)
        .expect("the drain ends");
    assert_eq!(ended - 1_000, DRAIN_LIMIT_SECONDS);
    assert_eq!(drain.step(3, ended), Step::Done { left: 3 });
}

#[test]
fn a_drain_with_no_request_in_progress_ends_at_once() {
    assert_eq!(Drain::new(60, 5).step(0, 5), Step::Done { left: 0 });
}

#[test]
fn a_drain_waits_while_requests_are_in_progress_before_its_seconds() {
    let drain = Drain::new(10, 0);
    assert_eq!(drain.step(1, 9), Step::Wait);
    assert_eq!(drain.step(1, 10), Step::Done { left: 1 });
}
