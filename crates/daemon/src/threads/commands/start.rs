//! A checked start in the holder, including its optional relay lease (TH118).

use super::refused;
use crate::threads::holder::{Shared, locked};
use crate::threads::program::{self, Start};
use crate::threads::{
    tree,
    wire::{Action, Outcome, Refusal, Stream, ThreadKey},
};
use std::sync::{Arc, Mutex};

pub(super) async fn run(shared: &Arc<Mutex<Shared>>, action: Action) -> Outcome {
    let Action::Start {
        relay,
        thread,
        program,
        args,
        env,
        folder,
    } = action
    else {
        return refused(Refusal::BadStart);
    };
    let env = super::marked(env, &thread);
    let start = Start {
        program: &program,
        args: &args,
        env: &env,
        folder: &folder,
    };
    start_thread(shared, thread, &start, relay).await
}

async fn start_thread(
    shared: &Arc<Mutex<Shared>>,
    thread: ThreadKey,
    start: &Start<'_>,
    relay: Option<agent_threads::drive::Program>,
) -> Outcome {
    {
        let Ok(state) = shared.lock() else {
            return refused(Refusal::StartFailed);
        };
        if state.draining || state.closing {
            return refused(Refusal::Version);
        }
        if state.threads.contains_key(&thread) {
            return refused(Refusal::ThreadExists);
        }
    }
    if !start.fits() {
        return refused(Refusal::BadStart);
    }
    let lease = if let Some(program) = relay {
        if let Err(why) = program::relay::permitted(program, start).await {
            return refused(why);
        }
        let relays = Arc::clone(&locked(shared).relays);
        match relays.open(&thread, program).await {
            Ok(lease) => Some(lease),
            Err(why) => return refused(why),
        }
    } else {
        None
    };
    let spawned = match (&lease, relay) {
        (Some(lease), Some(program)) => lease.launch(program, start),
        _ => start.spawn(),
    };
    let Ok(child) = spawned else {
        return refused(Refusal::StartFailed);
    };
    attach(shared, thread, child, lease).await
}

async fn attach(
    shared: &Arc<Mutex<Shared>>,
    thread: ThreadKey,
    mut child: tokio::process::Child,
    lease: Option<program::relay::Lease>,
) -> Outcome {
    let pid = child.id().expect("a spawned child has its pid before wait");
    let started = tree::started(pid).await;
    let input = child.stdin.take().map(program::input);
    let (Some(out), Some(err)) = (child.stdout.take(), child.stderr.take()) else {
        return refused(Refusal::StartFailed);
    };
    if let Ok(mut state) = shared.lock() {
        let mut slot = state.new_slot(pid, started, input);
        slot.relay = lease;
        state.threads.insert(thread.clone(), slot);
        state.write_record();
    }
    let readers = [
        tokio::spawn(program::read(
            Arc::clone(shared),
            thread.clone(),
            Stream::Out,
            out,
        )),
        tokio::spawn(program::read(
            Arc::clone(shared),
            thread.clone(),
            Stream::Err,
            err,
        )),
    ];
    tokio::spawn(program::wait(Arc::clone(shared), thread, child, readers));
    Outcome::Started { pid }
}
