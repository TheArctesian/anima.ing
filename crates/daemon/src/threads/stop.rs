//! The stop of a program and of the processes below it (THR-049): the walk of the tree, the
//! sweep of what was seen below the program, and the processes that carry the mark of the
//! thread.

use std::collections::BTreeSet;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio::sync::OwnedMutexGuard;

use super::commands::refused;
use super::holder::{Shared, Slot, locked};
use super::marker::Keep;
use super::program;
use super::tree;
use super::wire::{Outcome, Refusal, ThreadKey};

/// The time the program of a thread has after `SIGTERM` before `SIGKILL`.
const GRACE: Duration = Duration::from_secs(3);

/// What the holder knows of a program when it ends its processes.
struct Ending {
    root: u32,
    /// The start of the program, when only its children end.
    since: Option<String>,
    gone: bool,
    spare: tree::Known,
    /// The processes seen below the program that may have left its tree since (a stop only).
    seen: tree::Known,
    left_behind: bool,
}

impl Ending {
    /// What the holder knows of the program in `slot` for an end of its children, or of it too
    /// when `with_root`.
    fn of(slot: &Slot, with_root: bool) -> Self {
        Self {
            root: slot.pid,
            // An unknown start reads as no time, so that the sweep after a turn ends nothing.
            since: (!with_root).then(|| slot.started.clone().unwrap_or_default()),
            gone: slot.reaped || slot.exit.is_some(),
            spare: if with_root {
                tree::Known::new()
            } else {
                slot.spared.clone()
            },
            // A stop sweeps what was seen; so does each command on a program that ended and
            // left a process that a sweep could not end.
            seen: if with_root || slot.left_behind {
                slot.seen.clone()
            } else {
                tree::Known::new()
            },
            left_behind: slot.left_behind,
        }
    }

    /// What the sweep by mark keeps; `program` when only the children end.
    fn keep(&self, program: Option<u32>) -> Keep<'_> {
        Keep {
            program,
            since: self.since.as_deref(),
            spare: &self.spare,
        }
    }
}

/// A stop after its signals, before the wait for what got `SIGKILL` to be gone.
pub(super) struct Stopping {
    thread: ThreadKey,
    ending: Ending,
    killed: tree::Killed,
}

/// Sends the signals that end the processes of the program of `thread`, and the program too when
/// `with_root`; `Err` is the answer when the holder does not know the thread.
async fn kill(
    shared: &Arc<Mutex<Shared>>,
    thread: &ThreadKey,
    with_root: bool,
) -> Result<Stopping, Outcome> {
    let found = locked(shared)
        .threads
        .get(thread)
        .map(|slot| Ending::of(slot, with_root));
    let Some(ending) = found else {
        return Err(refused(Refusal::UnknownThread));
    };
    let killed = if ending.gone {
        // What a program that ended left: when its sweep did not prove that all ended, sweep
        // again; then the processes that carry its mark.
        let swept = if ending.left_behind {
            tree::kill_seen(&ending.seen).await
        } else {
            tree::Killed::nothing()
        };
        swept.and(super::marker::kill_marked(thread, &ending.keep(None)).await)
    } else {
        let ended = tree::kill_tree(ending.root, with_root, &ending.spare, GRACE).await;
        // A process that was below the program and left its tree (its parent ended) is found by
        // its start time (THR-049).
        let swept = tree::kill_seen(&ending.seen).await;
        // A process that left the tree between two looks is found by its mark. The ending of
        // the children keeps the program, which carries the mark too.
        let keep = ending.keep((!with_root).then_some(ending.root));
        let marked = super::marker::kill_marked(thread, &keep).await;
        ended.and(swept).and(marked)
    };
    Ok(Stopping {
        thread: thread.clone(),
        ending,
        killed,
    })
}

impl Stopping {
    /// The answer of the stop when `left` still run after the wait. The thread is looked at
    /// again: during the wait, a different command may have forgotten it or started a new
    /// program under its key.
    fn proved(self, shared: &Arc<Mutex<Shared>>, left: &BTreeSet<u32>) -> Outcome {
        let ended = self.killed.proved(left);
        if self.ending.gone && self.ending.left_behind {
            let mut state = locked(shared);
            let same = state
                .threads
                .get_mut(&self.thread)
                .filter(|slot| slot.pid == self.ending.root);
            if let Some(slot) = same {
                program::settle_sweep(slot, ended.all);
            }
            // A leftover that is proved ended leaves the record; one that is not stays for the
            // next holder.
            state.write_record();
        }
        Outcome::Stopped {
            ended: ended.count,
            all_ended: ended.all,
        }
    }
}

/// Ends the processes of the program of `thread`, and the program too when `with_root`. The
/// signals go while `turn` is held; the wait for the killed processes to be gone runs after it
/// is let go, so that the other commands of the holder do not wait for it (THR-049).
pub(super) async fn end(
    shared: &Arc<Mutex<Shared>>,
    thread: &ThreadKey,
    with_root: bool,
    turn: Option<OwnedMutexGuard<()>>,
) -> Outcome {
    let stopping = match kill(shared, thread, with_root).await {
        Ok(stopping) => stopping,
        Err(outcome) => return outcome,
    };
    drop(turn);
    let left = tree::left_running(&stopping.killed.pids).await;
    stopping.proved(shared, &left)
}

/// Ends each program the holder holds, with the processes below it (THR-049), at the end of the
/// holder. Each thread gets its signals at the same time, and all wait one time together, so the
/// end of the holder takes one wait, not one for each thread, before it writes the record.
pub async fn end_all(shared: &Arc<Mutex<Shared>>) {
    let running: Vec<ThreadKey> = locked(shared)
        .threads
        .iter()
        // A program that ended but left a process its sweep could not end is swept again.
        .filter(|(_, slot)| slot.exit.is_none() || slot.left_behind)
        .map(|(key, _)| key.clone())
        .collect();
    let mut kills = tokio::task::JoinSet::new();
    for thread in running {
        super::commands::close_input(shared, &thread);
        let shared = Arc::clone(shared);
        kills.spawn(async move { kill(&shared, &thread, true).await.ok() });
    }
    let mut stopping = Vec::new();
    while let Some(done) = kills.join_next().await {
        if let Ok(Some(one)) = done {
            stopping.push(one);
        }
    }
    let pids: BTreeSet<u32> = stopping
        .iter()
        .flat_map(|one| one.killed.pids.iter().copied())
        .collect();
    let left = tree::left_running(&pids).await;
    for one in stopping {
        one.proved(shared, &left);
    }
    locked(shared).write_record();
}
