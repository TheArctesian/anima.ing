use std::time::Duration;

use tokio::sync::mpsc;
use tokio::time::Instant;

use crate::pin::Verdict;

const QUEUE: usize = 512;

#[derive(Clone, Debug)]
pub enum Event {
    Served {
        server: String,
        path: String,
        query: String,
        cookie: String,
    },
    Report {
        server: String,
        name: String,
        body: serde_json::Value,
    },
    Invoked { label: String, from: String },
    Navigation {
        label: String,
        url: String,
        allowed: bool,
    },
    Challenge {
        label: String,
        host: String,
        port: u16,
        verdict: Verdict,
        system_trusted: bool,
    },
    Hook { result: Result<String, String> },
}

#[derive(Clone)]
pub struct Events(mpsc::Sender<Event>);

impl Events {
    pub fn send(&self, event: Event) {
        if self.0.try_send(event).is_err() {
            eprintln!("od7: event queue full or closed; an event was dropped");
        }
    }
}

pub struct Journal {
    queue: mpsc::Receiver<Event>,
    seen: Vec<Event>,
}

pub fn channel() -> (Events, Journal) {
    let (sender, queue) = mpsc::channel(QUEUE);
    (
        Events(sender),
        Journal {
            queue,
            seen: Vec::new(),
        },
    )
}

impl Journal {
    pub fn mark(&mut self) -> usize {
        self.drain();
        self.seen.len()
    }

    pub async fn until(
        &mut self,
        from: usize,
        wait: Duration,
        matches: impl Fn(&Event) -> bool,
    ) -> Option<Event> {
        let deadline = Instant::now() + wait;
        loop {
            self.drain();
            if let Some(found) = self.seen[from..].iter().find(|event| matches(event)) {
                return Some(found.clone());
            }
            match tokio::time::timeout_at(deadline, self.queue.recv()).await {
                Ok(Some(event)) => self.seen.push(event),
                Ok(None) | Err(_) => return None,
            }
        }
    }

    pub async fn settle(&mut self, wait: Duration) {
        let deadline = Instant::now() + wait;
        while let Ok(Some(event)) = tokio::time::timeout_at(deadline, self.queue.recv()).await {
            self.seen.push(event);
        }
    }

    pub fn since(&mut self, from: usize, matches: impl Fn(&Event) -> bool) -> Vec<Event> {
        self.drain();
        self.seen[from..]
            .iter()
            .filter(|event| matches(event))
            .cloned()
            .collect()
    }

    fn drain(&mut self) {
        while let Ok(event) = self.queue.try_recv() {
            self.seen.push(event);
        }
    }
}

pub fn is_report(event: &Event, server: &str, name: &str, who: Option<&str>) -> bool {
    match event {
        Event::Report {
            server: s,
            name: n,
            body,
        } => {
            s == server
                && n == name
                && who.is_none_or(|w| body.get("who").and_then(|v| v.as_str()) == Some(w))
        }
        _ => false,
    }
}
