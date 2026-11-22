//! How many refusals go to the daemon's log in one minute (MET-019): a process that sends refused
//! calls in a loop must not push the other lines out of the 1 MiB file. Pure: the minute is a
//! parameter. `runner.log` (THR-044) keeps each refusal; only the daemon's log is bounded.

use std::collections::HashMap;

use agent_threads::control::Refusal;
use uuid::Uuid;

/// The most refusal lines of one thread (or of calls with no thread) in one minute.
pub const PER_THREAD: u32 = 10;

/// The most refusal lines of all threads in one minute; it also bounds the map of counts.
pub const IN_ALL: u32 = 100;

/// What the log says of one refusal.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Say {
    /// The line of the refusal.
    Line,
    /// The line, and that more refusals of this minute are not logged.
    LastLine,
    /// Nothing: the bound of this minute is reached.
    Nothing,
}

/// The counts of the current minute.
#[derive(Debug, Default)]
pub struct RefusalLines {
    minute: i64,
    all: u32,
    counts: HashMap<Option<Uuid>, u32>,
}

impl RefusalLines {
    /// Counts one refusal of `thread` in `minute` and says what the log writes.
    pub fn admit(&mut self, thread: Option<Uuid>, minute: i64) -> Say {
        if minute != self.minute {
            self.minute = minute;
            self.all = 0;
            self.counts.clear();
        }
        if self.all >= IN_ALL {
            return Say::Nothing;
        }
        let count = self.counts.entry(thread).or_insert(0);
        if *count >= PER_THREAD {
            return Say::Nothing;
        }
        *count += 1;
        self.all += 1;
        if *count == PER_THREAD || self.all == IN_ALL {
            Say::LastLine
        } else {
            Say::Line
        }
    }
}

/// The fixed word of a refusal for the logs (LOG-007): never text of the person or of the program.
pub(in crate::threads::runner) const fn refusal_word(why: &Refusal) -> &'static str {
    match why {
        Refusal::UnknownThread => "unknown thread",
        Refusal::FromAnAgent => "from a process of an agent",
        Refusal::Running => "the thread runs",
        Refusal::FolderNotPermitted => "folder not permitted",
        Refusal::ProgramNotFound => "program not found",
        Refusal::ProgramDoesNotAnswer { .. } => "program did not answer",
        Refusal::BadValue => "bad value",
        Refusal::RequestGone => "request gone",
        Refusal::AnswerDoesNotFit => "answer does not fit",
        Refusal::NotRunning => "not running",
        Refusal::Holder => "holder",
        Refusal::HolderOlder { .. } => "holder older than the daemon",
        Refusal::OverLimit { .. } => "over the soft limit",
        Refusal::QueueFull => "queue full",
        Refusal::CannotChange { .. } => "the program cannot change that",
        Refusal::TooLarge { .. } => "a file is too large",
        Refusal::CannotAttach { .. } => "a file cannot go",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn met_019_a_loop_of_refusals_of_one_thread_gives_ten_lines_a_minute() {
        let mut lines = RefusalLines::default();
        let thread = Some(Uuid::now_v7());
        let said: Vec<Say> = (0..15).map(|_| lines.admit(thread, 1)).collect();
        assert_eq!(said.iter().filter(|say| **say == Say::Line).count(), 9);
        assert_eq!(said[9], Say::LastLine);
        assert!(said[10..].iter().all(|say| *say == Say::Nothing));
        assert_eq!(
            lines.admit(Some(Uuid::now_v7()), 1),
            Say::Line,
            "another thread"
        );
        assert_eq!(lines.admit(thread, 2), Say::Line, "the next minute");
    }

    #[test]
    fn met_019_new_thread_ids_in_a_loop_stop_at_the_bound_of_all() {
        let mut lines = RefusalLines::default();
        let said: Vec<Say> = (0..150)
            .map(|_| lines.admit(Some(Uuid::now_v7()), 7))
            .collect();
        assert_eq!(said[99], Say::LastLine);
        assert!(said[100..].iter().all(|say| *say == Say::Nothing));
        assert!(lines.counts.len() <= IN_ALL as usize);
    }
}
