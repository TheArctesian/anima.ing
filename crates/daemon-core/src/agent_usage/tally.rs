//! The counts read so far from one session file, kept so the daemon reads each file from where it stopped (AIS-001, K8 Section 4).

use std::collections::BTreeMap;

use chrono::{DateTime, NaiveDate, Timelike as _, Utc};

use super::{MAX_COUNT, Source, Tokens, claude, codex, model_or_other};

/// The counts of one UTC day of a session.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Day {
    pub prompts: u64,
    pub tool_calls: u64,
    /// Each key is a model name or `other`.
    pub models: BTreeMap<String, Tokens>,
}

/// What one session file gave so far; it keeps no text of a line.
#[derive(Clone, Debug)]
pub struct Tally {
    counts: Counts,
    lines: Lines,
}

#[derive(Clone, Debug, Default)]
pub(super) struct Counts {
    days: BTreeMap<NaiveDate, Day>,
    span: Option<(DateTime<Utc>, DateTime<Utc>)>,
    cwd: Option<String>,
}

#[derive(Clone, Debug)]
enum Lines {
    Claude(claude::Recent),
    Codex(codex::Thread),
}

impl Tally {
    #[must_use]
    pub fn new(source: Source) -> Self {
        let lines = match source {
            Source::ClaudeCode => Lines::Claude(claude::Recent::default()),
            Source::Codex => Lines::Codex(codex::Thread::default()),
        };
        Self {
            counts: Counts::default(),
            lines,
        }
    }

    /// Reads one line without its newline; a line it cannot read adds nothing.
    pub fn feed(&mut self, line: &[u8]) {
        match &mut self.lines {
            Lines::Claude(recent) => claude::read(recent, &mut self.counts, line),
            Lines::Codex(thread) => codex::read(thread, &mut self.counts, line),
        }
    }

    #[must_use]
    pub const fn source(&self) -> Source {
        match self.lines {
            Lines::Claude(_) => Source::ClaudeCode,
            Lines::Codex(_) => Source::Codex,
        }
    }

    #[must_use]
    pub const fn days(&self) -> &BTreeMap<NaiveDate, Day> {
        &self.counts.days
    }

    #[must_use]
    pub const fn span(&self) -> Option<(DateTime<Utc>, DateTime<Utc>)> {
        self.counts.span
    }

    /// The first working folder the file named; it is checked against the exclusions and never sent.
    #[must_use]
    pub fn cwd(&self) -> Option<&str> {
        self.counts.cwd.as_deref()
    }

    /// The Codex thread and its parent for a sub-agent; `None` for a Claude Code file, named by its path.
    #[must_use]
    pub fn thread(&self) -> Option<(&str, Option<&str>)> {
        match &self.lines {
            Lines::Codex(thread) => thread.ids(),
            Lines::Claude(_) => None,
        }
    }
}

impl Counts {
    pub(super) fn saw(&mut self, at: DateTime<Utc>) {
        self.span = Some(match self.span {
            Some((first, last)) => (first.min(at), last.max(at)),
            None => (at, at),
        });
    }

    pub(super) fn named_folder(&mut self, cwd: &str) {
        if self.cwd.is_none() && !cwd.is_empty() {
            self.cwd = Some(cwd.to_owned());
        }
    }

    pub(super) fn prompt(&mut self, at: DateTime<Utc>) {
        let day = self.day(at);
        day.prompts = capped(day.prompts, 1);
    }

    pub(super) fn tool_calls(&mut self, at: DateTime<Utc>, count: u64) {
        if count > 0 {
            let day = self.day(at);
            day.tool_calls = capped(day.tool_calls, count);
        }
    }

    pub(super) fn tokens(&mut self, at: DateTime<Utc>, model: &str, tokens: Tokens) {
        let day = self.day(at);
        let kept = day
            .models
            .entry(model_or_other(model).to_owned())
            .or_default();
        *kept = kept.plus(tokens);
    }

    fn day(&mut self, at: DateTime<Utc>) -> &mut Day {
        self.days.entry(at.date_naive()).or_default()
    }
}

fn capped(count: u64, more: u64) -> u64 {
    count.saturating_add(more).min(MAX_COUNT)
}

pub(super) fn time(text: &str) -> Option<DateTime<Utc>> {
    let at = DateTime::parse_from_rfc3339(text).ok()?.with_timezone(&Utc);
    at.with_nanosecond(0)
}
