//! What the holder does for each command of the daemon. A command is done one time: the same
//! ID again gets the first answer and does nothing (H10).

use std::sync::{Arc, Mutex};

use tokio::sync::OwnedMutexGuard;

use super::holder::{Shared, locked};
mod start;
use super::tree;
use super::wire::{self, Action, Command, Event, Outcome, Refusal, ThreadKey};

/// Does `command` and gives its answer to the daemon. `turn` lets one command run at a time; a
/// stop lets it go while it waits for the killed processes to be gone ([`super::stop::end`]).
pub async fn run(
    shared: &Arc<Mutex<Shared>>,
    command: Command,
    turn: OwnedMutexGuard<()>,
) -> Outcome {
    // `Hello` is done each time: it is how a daemon that starts again gets the lines it missed.
    let repeatable = matches!(command.action, Action::Hello { .. });
    let known = if repeatable {
        None
    } else {
        let state = locked(shared);
        if state.acting.contains(&command.id) {
            // The same command, sent again while it waits without the turn: it is done one
            // time, and its answer goes out when it is done (H10).
            return Outcome::Ok;
        }
        state.done.get(&command.id).cloned()
    };
    let outcome = if let Some(outcome) = known {
        outcome
    } else {
        locked(shared).acting.insert(command.id);
        let mut turn = Some(turn);
        let outcome = act(shared, command.action, &mut turn).await;
        let mut state = locked(shared);
        state.acting.remove(&command.id);
        state.remember(command.id, outcome.clone());
        outcome
    };
    locked(shared).send(&Event::Done {
        id: command.id,
        outcome: outcome.clone(),
    });
    outcome
}

async fn act(
    shared: &Arc<Mutex<Shared>>,
    action: Action,
    turn: &mut Option<OwnedMutexGuard<()>>,
) -> Outcome {
    match action {
        Action::Hello { version, git_reads } => hello(shared, version, git_reads),
        action @ Action::Start { .. } => start::run(shared, action).await,
        Action::Write { thread, line } => write(shared, &thread, line),
        Action::CloseInput { thread } => {
            if close_input(shared, &thread) {
                Outcome::Ok
            } else {
                refused(Refusal::UnknownThread)
            }
        }
        Action::Mark { thread } => mark(shared, &thread).await,
        Action::Ack { thread, seq } => {
            let Ok(mut state) = shared.lock() else {
                return refused(Refusal::UnknownThread);
            };
            match state.threads.get_mut(&thread) {
                Some(slot) => {
                    slot.outbox.ack(seq);
                    Outcome::Ok
                }
                None => refused(Refusal::UnknownThread),
            }
        }
        Action::EndChildren { thread } => {
            super::stop::end(shared, &thread, false, turn.take()).await
        }
        Action::Stop { thread } => {
            locked(shared).revoke_git_read(&thread);
            close_input(shared, &thread);
            super::stop::end(shared, &thread, true, turn.take()).await
        }
        Action::Forget { thread } => forget(shared, &thread),
        Action::Quit => {
            // A stop that still waits for its processes (it lets the turn go) holds the end of
            // the holder too: its answer goes out first.
            let busy = shared.lock().is_ok_and(|state| {
                state.acting.len() > 1 || state.threads.values().any(|slot| slot.exit.is_none())
            });
            if busy {
                refused(Refusal::Busy)
            } else {
                Outcome::Ok
            }
        }
    }
}

/// The environment of a start with the mark of `thread` last, so that no value of the daemon
/// takes its place (THR-049).
fn marked(mut env: Vec<(String, String)>, thread: &ThreadKey) -> Vec<(String, String)> {
    env.retain(|(name, _)| name != super::marker::MARK);
    env.push(super::marker::entry(thread));
    env
}

pub(super) fn refused(why: Refusal) -> Outcome {
    Outcome::Refused { why }
}

/// Answers with the version and the threads, then gives again each line the daemon did not
/// confirm and each end it did not see.
fn hello(shared: &Arc<Mutex<Shared>>, version: u32, git_reads: bool) -> Outcome {
    let Ok(mut state) = shared.lock() else {
        return Outcome::Ok;
    };
    state.draining = version != wire::VERSION;
    let git_read_roots = state.git_read_hello(git_reads);
    state.send(&Event::Hello {
        version: wire::VERSION,
        threads: state.held(),
        build: wire::this_build(),
        git_read_roots,
    });
    for (thread, slot) in &state.threads {
        for (first, lines) in slot.outbox.pending() {
            state.send(&Event::Lines {
                thread: thread.clone(),
                first,
                lines,
            });
        }
        if slot.waiting {
            state.send(&Event::Waiting {
                thread: thread.clone(),
            });
        }
        if let Some(exit) = slot.exit {
            state.send(&Event::Exited {
                thread: thread.clone(),
                exit,
            });
        }
    }
    Outcome::Ok
}

/// Closes the input of the program of `thread`; false when the holder does not know it.
pub(super) fn close_input(shared: &Arc<Mutex<Shared>>, thread: &ThreadKey) -> bool {
    let Ok(mut state) = shared.lock() else {
        return false;
    };
    state
        .threads
        .get_mut(thread)
        .map(|slot| slot.input = None)
        .is_some()
}

/// Puts `line` in the queue of the input of the program. It never waits: a program that does
/// not read its input gets [`Refusal::InputFull`], and the other commands go on (THR-113).
fn write(shared: &Arc<Mutex<Shared>>, thread: &ThreadKey, mut line: String) -> Outcome {
    use tokio::sync::mpsc::error::TrySendError;
    if !line.ends_with('\n') {
        line.push('\n');
    }
    let Ok(state) = shared.lock() else {
        return refused(Refusal::UnknownThread);
    };
    let Some(slot) = state.threads.get(thread) else {
        return refused(Refusal::UnknownThread);
    };
    if let Some(lease) = &slot.relay {
        line = match lease.input(line) {
            Ok(line) => line,
            Err(why) => return refused(why),
        };
    }
    match slot.input.as_ref().map(|input| input.try_send(line)) {
        Some(Ok(())) => Outcome::Ok,
        Some(Err(TrySendError::Full(_))) => refused(Refusal::InputFull),
        Some(Err(TrySendError::Closed(_))) | None => refused(Refusal::InputClosed),
    }
}

/// Keeps the processes below the program now as spared (see [`Action::Mark`]).
async fn mark(shared: &Arc<Mutex<Shared>>, thread: &ThreadKey) -> Outcome {
    let root = shared
        .lock()
        .ok()
        .and_then(|state| state.threads.get(thread).map(|slot| slot.pid));
    let Some(root) = root else {
        return refused(Refusal::UnknownThread);
    };
    let below = tree::table()
        .await
        .map_or_else(tree::Known::new, |table| table.below(root));
    if let Ok(mut state) = shared.lock()
        && let Some(slot) = state.threads.get_mut(thread)
    {
        slot.spared = below;
    }
    Outcome::Ok
}

fn forget(shared: &Arc<Mutex<Shared>>, thread: &ThreadKey) -> Outcome {
    let Ok(mut state) = shared.lock() else {
        return refused(Refusal::UnknownThread);
    };
    match state.threads.get(thread).map(|slot| slot.exit.is_some()) {
        None => refused(Refusal::UnknownThread),
        Some(false) => refused(Refusal::Busy),
        Some(true) => {
            // What the program left and its sweep could not prove ended stays in the record, so
            // that the next start of a holder sweeps it again (THR-049).
            if let Some(slot) = state.threads.remove(thread)
                && slot.left_behind
            {
                state.inherited.push(super::record::Entry {
                    thread: thread.clone(),
                    pid: slot.pid,
                    started: slot.started,
                    left: Some(slot.seen),
                });
                state.write_record();
            }
            Outcome::Ok
        }
    }
}
