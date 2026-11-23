//! Each synthetic prompt, title, path and account holds [`MARKER`], and the tests prove it never
//! reaches what the daemon sends (AIS-018, AIS-022).

use super::{Session, SessionDay, Source, Tally};
use crate::wire::BatchAgentUsage;

mod claude;
mod codex;
mod repository;
mod session;

const MARKER: &str = "MARKER-7f3a9c-SECRET";

fn tally(source: Source, lines: &str) -> Tally {
    let mut tally = Tally::new(source);
    for line in lines.lines() {
        tally.feed(line.as_bytes());
    }
    tally
}

fn days_of(tally: &Tally, id: &str) -> Vec<SessionDay> {
    let mut session = Session::new(tally.source(), id, None);
    session.add(tally);
    session.session_days()
}

fn assert_no_marker(tally: &Tally, id: &str) {
    let days = days_of(tally, id);
    let json = serde_json::to_string(&BatchAgentUsage::new(super::UsageState::Reading, &days))
        .expect("serialises");
    assert!(!json.contains(MARKER), "the batch holds session text");
    assert!(!format!("{days:?}").contains(MARKER));
    let kept = format!("{:?} {:?} {:?}", tally.days(), tally.span(), tally.thread());
    assert!(!kept.contains(MARKER), "the tally keeps session text");
}
