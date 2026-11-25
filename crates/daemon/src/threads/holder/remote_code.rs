//! Holder authority for K10 launch. The pending timed exchange belongs to one socket connection.

use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};

use agent_threads::drive::{Launch, Program, SessionStart};

use super::{Shared, locked};
use crate::{
    config::home::Home,
    readers::agent_sessions::user_home,
    remote_code::{
        Store,
        broker::{Matched, Pending, Phase},
        launch::{Dispatch, LiveLease, Progress, Spawned},
        policy::{Policy, Refusal},
        wire::{DispatchRequest, NotCreated},
    },
    threads::{
        commands::{marked, start::attach},
        program::Start,
        wire::remote_code::{Start as Order, ToHolder, ToRunner},
    },
};

pub(super) struct Exchange {
    order: Box<Order>,
    pending: Pending,
    lease: Option<LiveLease>,
}

pub(super) struct PendingChild {
    request: DispatchRequest,
    child: Box<Spawned>,
}

pub(super) async fn run(
    shared: &Arc<Mutex<Shared>>,
    (exchange, generation): (&mut Option<Exchange>, u64),
    frame: ToHolder,
) {
    let Some(home) = locked(shared)
        .dir
        .parent()
        .map(|dir| Home::at(dir.to_owned()))
    else {
        return;
    };
    let store = Store::at(&home);
    let response = match frame {
        ToHolder::Reserve { claim } => home
            .read_config()
            .ok()
            .flatten()
            .and_then(|config| store.reserve((config.device, &config.hub), &claim).ok())
            .map_or(ToRunner::Refused, |saved| ToRunner::Reserved {
                request: *saved.request(),
            }),
        ToHolder::Start(order) if exchange.is_none() => {
            let request = order.request;
            if let Some((pending, ask)) = Pending::begin(generation, Phase::Prepare, request) {
                *exchange = Some(Exchange {
                    order,
                    pending,
                    lease: None,
                });
                ToRunner::Ask { ask }
            } else {
                ToRunner::Refused
            }
        }
        ToHolder::Reply { reply } => {
            let Some(response) = accept_reply(shared, &home, (exchange, generation), reply).await
            else {
                return;
            };
            response
        }
        ToHolder::Close { request, cause } => {
            *exchange = None;
            close(&store, request, cause)
        }
        ToHolder::Reset { request } => {
            *exchange = None;
            ToRunner::Pending { request }
        }
        ToHolder::Recover { request } => recover(shared, &store, request).await,
        ToHolder::Start(_) => ToRunner::Refused,
    };
    if let Some(event) = response.event() {
        locked(shared).send(&event);
    }
}

async fn accept_reply(
    shared: &Arc<Mutex<Shared>>,
    home: &Home,
    (exchange, generation): (&mut Option<Exchange>, u64),
    reply: crate::remote_code::broker::Reply,
) -> Option<ToRunner> {
    let store = Store::at(home);
    if !exchange.as_ref()?.pending.matches(generation, &reply) {
        return None;
    }
    let Exchange {
        order,
        pending,
        lease,
    } = exchange.take()?;
    Some(match pending.receive(generation, reply) {
        Some(Matched::Live(lease)) => {
            let (pending, ask) = Pending::begin(generation, Phase::Authorize, order.request)?;
            *exchange = Some(Exchange {
                order,
                pending,
                lease: Some(lease),
            });
            ToRunner::Ask { ask }
        }
        Some(Matched::Authorized(grant)) => {
            launch(shared, home, order, (grant, lease.as_ref()?)).await
        }
        Some(Matched::TargetUnavailable) => close(&store, order.request, NotCreated::Refused),
        Some(Matched::Stopped(_) | Matched::Final { .. }) => {
            let cause = if lease.is_some() {
                NotCreated::Expired
            } else {
                NotCreated::Stopped
            };
            close(&store, order.request, cause)
        }
        _ => ToRunner::Pending {
            request: order.request,
        },
    })
}

fn close(store: &Store, request: DispatchRequest, cause: NotCreated) -> ToRunner {
    if matches!(store.close(&request, cause), Ok(Progress::Closed(_))) {
        ToRunner::Refused
    } else {
        ToRunner::Pending { request }
    }
}

async fn launch(
    shared: &Arc<Mutex<Shared>>,
    home: &Home,
    order: Box<Order>,
    authority: (crate::remote_code::launch::DispatchGrant, &LiveLease),
) -> ToRunner {
    let request = order.request;
    let store = Store::at(home);
    let Some((command, user)) = prepare(shared, &store, &order) else {
        return close(&store, request, NotCreated::Refused);
    };
    let guard = |saved: &crate::remote_code::launch::Reservation| {
        let config = home
            .read_config()
            .ok()
            .flatten()
            .ok_or(Refusal::ThreadsOff)?;
        if !saved.enrolled_at(config.device, &config.hub) {
            return Err(Refusal::ThreadsOff);
        }
        let policy = Policy::read(home, user.clone());
        let targets = policy.targets(order.candidates.clone());
        policy.check_launch(saved.project(), saved.parameters(), &targets)
    };
    match store.spawn(authority, guard, (order.mode, command)) {
        Ok(Dispatch::Spawned(child)) => {
            locked(shared).remote_child = Some(PendingChild { request, child });
            recover(shared, &store, request).await
        }
        Ok(Dispatch::NoChild(Progress::Closed(_))) => ToRunner::Refused,
        _ => ToRunner::Pending { request },
    }
}

fn prepare(
    shared: &Mutex<Shared>,
    store: &Store,
    order: &Order,
) -> Option<(tokio::process::Command, PathBuf)> {
    let saved = store.reservation(&order.request).ok()?;
    let parameters = saved.parameters();
    let state = locked(shared);
    if state.draining
        || state.closing
        || state.remote_child.is_some()
        || state
            .threads
            .contains_key(&order.request.runner.to_string())
        || !matches!(parameters.program, Program::ClaudeCode | Program::Codex)
    {
        return None;
    }
    drop(state);
    let user = user_home(|name| {
        order
            .env
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.clone())
    })?;
    let invocation = Launch {
        program: parameters.program,
        path: PathBuf::from(&order.program),
        mode: order.mode,
        model: None,
        effort: None,
        session: SessionStart::New(order.request.runner),
    }
    .invocation()
    .ok()?;
    let program = invocation.program.to_str()?;
    let env = marked(order.env.clone(), &order.request.runner.to_string());
    let start = Start {
        program,
        args: &invocation.args,
        env: &env,
        folder: parameters.folder.to_str()?,
    };
    start.fits().then(|| (start.command(), user))
}

async fn recover(shared: &Arc<Mutex<Shared>>, store: &Store, request: DispatchRequest) -> ToRunner {
    let Ok(reserved) = store.reservation(&request) else {
        return ToRunner::Pending { request };
    };
    let key = request.runner.to_string();
    let pending = {
        let mut state = locked(shared);
        if let Some(slot) = state.threads.get(&key)
            && !slot.reaped
            && slot.exit.is_none()
            && let Some(process) = &slot.remote_process
            && reserved.matches_process(process)
        {
            return ToRunner::Created {
                request,
                process: process.clone(),
            };
        }
        if !state
            .remote_child
            .as_ref()
            .is_some_and(|pending| pending.request == request)
        {
            return ToRunner::Pending { request };
        }
        state.remote_child.take()
    };
    let Some(PendingChild { mut child, .. }) = pending else {
        return ToRunner::Pending { request };
    };
    if matches!(child.record(store), Ok(Progress::Created)) {
        match (*child).into_child() {
            Ok((child, process)) => {
                let _ = attach(shared, key, child, (None, Some(process.clone()))).await;
                return ToRunner::Created { request, process };
            }
            Err(pending) => child = pending,
        }
    }
    if !matches!(child.exited(), Ok(true)) {
        locked(shared).remote_child = Some(PendingChild { request, child });
    }
    ToRunner::Pending { request }
}

pub(super) async fn end_pending(shared: &Mutex<Shared>) {
    let pending = locked(shared).remote_child.take();
    if let Some(mut pending) = pending
        && let Err(error) = pending.child.end().await
    {
        tracing::warn!(%error, "could not reap a pending remote Code child");
    }
}
