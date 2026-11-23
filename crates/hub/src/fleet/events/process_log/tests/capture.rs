use std::{
    io,
    sync::{Arc, Mutex, Once},
};

use serde_json::Value;
use tracing::{Event, Metadata, Subscriber, span, subscriber::Interest};

use super::super::subscriber;

#[derive(Clone, Default)]
pub(crate) struct Capture(Arc<Mutex<Vec<u8>>>);

impl io::Write for Capture {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl Capture {
    pub(crate) fn text(&self) -> String {
        String::from_utf8(self.0.lock().unwrap().clone()).unwrap()
    }

    pub(crate) fn lines(&self) -> Vec<Value> {
        self.text()
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect()
    }
}

/// Answers "sometimes" for every callsite: interest is cached per process, so a thread with no
/// subscriber must not cache "never" while another thread captures.
struct Undecided;

impl Subscriber for Undecided {
    fn register_callsite(&self, _: &'static Metadata<'static>) -> Interest {
        Interest::sometimes()
    }

    fn enabled(&self, _: &Metadata<'_>) -> bool {
        false
    }

    fn new_span(&self, _: &span::Attributes<'_>) -> span::Id {
        span::Id::from_u64(1)
    }

    fn record(&self, _: &span::Id, _: &span::Record<'_>) {}

    fn record_follows_from(&self, _: &span::Id, _: &span::Id) {}

    fn event(&self, _: &Event<'_>) {}

    fn enter(&self, _: &span::Id) {}

    fn exit(&self, _: &span::Id) {}
}

/// Rebuilding the interest cache after the scoped subscriber is set makes earlier callsites ask again.
pub(crate) fn capture_log(filter: &str) -> (Capture, tracing::subscriber::DefaultGuard) {
    static UNDECIDED: Once = Once::new();
    UNDECIDED.call_once(|| {
        tracing::subscriber::set_global_default(Undecided).expect("no other global subscriber");
    });
    let capture = Capture::default();
    let writer = capture.clone();
    let log = subscriber(filter, move || writer.clone()).unwrap();
    let guard = tracing::subscriber::set_default(log);
    tracing::callsite::rebuild_interest_cache();
    (capture, guard)
}
