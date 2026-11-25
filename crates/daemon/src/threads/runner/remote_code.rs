//! Remote Code uses the runner's serialized event loop, apart from authenticated network I/O.

mod control;
mod launch;
mod polling;

use std::{collections::BTreeMap, time::Duration};

use tokio::{sync::mpsc, time::Instant};
use uuid::Uuid;

use crate::remote_code::{
    broker::Reply,
    command::Delivered,
    launch::{ProcessIdentity, Reservation},
    wire::{ClaimedLaunch, DispatchRequest},
};

#[derive(Default)]
pub(super) struct Remote {
    events: Option<mpsc::Sender<Event>>,
    active: Option<DispatchRequest>,
    reserving: bool,
    claiming: bool,
    holder: Holder,
    polling: bool,
    threads: Option<daemon_core::wire::Switch>,
    due_claim: Option<Instant>,
    due_poll: Option<Instant>,
    live: BTreeMap<Uuid, (DispatchRequest, ProcessIdentity)>,
    backed_off: crate::push::pacing::Backoff,
    publications: BTreeMap<Uuid, Instant>,
}

#[derive(Default, PartialEq, Eq)]
enum Holder {
    #[default]
    Unknown,
    Opening,
    Proving,
    Ready,
}

pub(super) enum Event {
    Look,
    Tick,
    Claim(Option<ClaimedLaunch>),
    Poll {
        enrollment: Option<crate::config::Config>,
        result: crate::hub::answer::Answer<Vec<Delivered>>,
    },
    Relay {
        request: DispatchRequest,
        result: crate::hub::answer::Answer<Reply>,
    },
}

impl Remote {
    pub(super) fn open(&mut self) -> mpsc::Receiver<Event> {
        let (sender, receiver) = mpsc::channel(32);
        self.events = Some(sender);
        receiver
    }

    pub(super) fn disconnect(&mut self) {
        self.active = None;
        self.reserving = false;
        self.holder = Holder::Unknown;
        self.live.clear();
    }

    fn busy(&self) -> bool {
        self.reserving || self.active.is_some()
    }

    fn delay_poll(&mut self, answer: &crate::hub::answer::Answer<Vec<Delivered>>) {
        let now = Instant::now();
        let backoff = if matches!(answer, crate::hub::answer::Answer::Accepted(_)) {
            self.backed_off.recovered();
            Duration::ZERO
        } else {
            self.backed_off.failed(0.5)
        };
        self.due_poll = Some(
            now + crate::remote_code::delivery::next_poll(answer, backoff)
                .unwrap_or(crate::push::pacing::REFUSED_RETRY),
        );
    }
}

fn request_of(saved: &Reservation) -> DispatchRequest {
    *saved.request()
}

pub(super) async fn next(
    events: &mut mpsc::Receiver<Event>,
    look: &mut tokio::time::Interval,
    remote: &mut tokio::time::Interval,
) -> Option<Event> {
    tokio::select! {
        event = events.recv() => event,
        _ = look.tick() => Some(Event::Look),
        _ = remote.tick() => Some(Event::Tick),
    }
}
