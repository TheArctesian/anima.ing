//! Serve the local daemon's hosting requests after peer and pair checks.
pub mod applications;
mod apply;
mod container;
mod pairing;
mod socket;
pub mod terminal;
mod wire;

pub(crate) use apply::verify_candidate;

use std::{
    io,
    os::unix::net::UnixStream,
    path::Path,
    sync::{
        Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

use anima_applications_wire::{Command, Message, Reply};

use crate::{
    Pair, Plan, Refusal, Work,
    execution::{Fault, files::Store, hosting, run_container},
    request,
    startup::{self, Files, Ready},
    work::container::action_plan,
};

use self::pairing::Pairing;

const CONNECTIONS: usize = 8;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Startup(#[from] startup::Error),
    #[error(transparent)]
    Storage(#[from] Fault),
    #[error("the applications listener could not continue ({0})")]
    Listener(io::ErrorKind),
}

struct Service<'a> {
    ready: &'a Ready,
    settings: &'a Path,
    pair: Mutex<Pairing<'a>>,
}

/// Serve the one configured daemon through `<socket-directory>/control.sock`.
///
/// # Errors
/// Startup authority checks fail, or the private state or listener cannot be opened.
pub fn serve(files: &Files<'_>, socket_directory: &Path) -> Result<(), Error> {
    let ready = startup::check(files)?;
    startup::socket_directory(socket_directory)?;
    let state = Path::new(&ready.paths().root).join("control");
    Store::open(Path::new(&ready.paths().root))?
        .folder(state.to_str().ok_or(Fault::UnsafePlan)?)?;
    let service = Service {
        ready: &ready,
        settings: files.settings,
        pair: Mutex::new(Pairing::open(&state)?),
    };
    let listener = socket::Listener::bind(socket_directory, ready.accepted_daemon())
        .map_err(|error| Error::Listener(error.kind()))?;
    let active = AtomicUsize::new(0);
    thread::scope(|scope| {
        loop {
            let Some(stream) = accept(&listener, ready.accepted_daemon())? else {
                continue;
            };
            let Some(slot) = Slot::claim(&active) else {
                continue;
            };
            let service = &service;
            thread::Builder::new()
                .spawn_scoped(scope, move || {
                    let _slot = slot;
                    service.respond(stream);
                })
                .map_err(|error| Error::Listener(error.kind()))?;
        }
    })
}

fn accept(listener: &socket::Listener, daemon: u32) -> Result<Option<UnixStream>, Error> {
    match listener.accept(daemon) {
        Err(error) if error.kind() == io::ErrorKind::Interrupted => Ok(None),
        result => result.map_err(|error| Error::Listener(error.kind())),
    }
}

impl Service<'_> {
    fn respond(&self, mut stream: UnixStream) {
        let reply = match wire::receive(&mut stream) {
            Ok(message) => self.handle(&mut stream, message),
            Err(_) => refused(Refusal::MemberUnknown),
        };
        let _ = wire::send(&mut stream, &reply);
    }

    fn handle(&self, stream: &mut UnixStream, message: Message) -> Reply {
        let start = Instant::now();
        if !self.enabled() {
            return refused(Refusal::ApplicationsOff);
        }
        let Some(pair) = Pair::of(&message.origin, &message.device) else {
            return refused(Refusal::MemberUnknown);
        };
        let apply_probe = matches!(message.command, Command::ProbeApply);
        let item = match message.command {
            Command::Probe | Command::ProbeApply => None,
            Command::Item { json } => match request::read(&json) {
                Ok(item) => Some(item),
                Err(cause) => return refused(cause),
            },
        };
        let Some(managed) = self.ready.settings().managed.as_ref() else {
            return refused(Refusal::ApplicationsOff);
        };
        if let Err(cause) = self.authorize(managed.set_on_at, &pair) {
            return refused(cause);
        }
        let probe = if apply_probe {
            if hosting::available(self.ready.paths()) {
                Reply::ApplyAvailable
            } else {
                refused(Refusal::Busy)
            }
        } else {
            Reply::Paired
        };
        item.map_or(probe, |item| {
            let authorized = || self.enabled() && self.authorize(managed.set_on_at, &pair).is_ok();
            if let Work::Apply(version) = &item.work {
                eprintln!(
                    "application_apply item={} application={} received",
                    item.id, version.application
                );
                let deadline = start + Duration::from_secs(item.seconds);
                let reply = apply::run((self.ready, version), stream, deadline, authorized);
                eprintln!("application_apply item={} result={reply:?}", item.id);
                return reply;
            }
            self.run(&item, start, |deadline| {
                wire::send(stream, &Reply::Ready).map_err(|_| failed(Fault::CommandFailed))?;
                wire::await_start(stream, deadline).map_err(|error| start_failed(error.kind()))?;
                if !authorized() {
                    return Err(failed(Fault::CommandFailed));
                }
                Ok(())
            })
        })
    }

    fn authorize(&self, set_on_at: u64, pair: &Pair) -> Result<(), Refusal> {
        self.pair
            .lock()
            .map_err(|_| Refusal::ApplicationsOff)?
            .accept(set_on_at, pair)
    }

    fn enabled(&self) -> bool {
        startup::settings_current(self.settings, self.ready.settings())
    }

    fn run(
        &self,
        item: &request::Request,
        start: Instant,
        await_start: impl FnOnce(Instant) -> Result<(), Reply>,
    ) -> Reply {
        let Work::Container {
            ref container,
            action,
        } = item.work
        else {
            return refused(Refusal::TypeUnknown);
        };
        let deadline = start + Duration::from_secs(item.seconds);
        eprintln!(
            "container_action item={} container={container} action={action:?} received",
            item.id
        );
        let reply = self.act((container, action), deadline, await_start);
        eprintln!("container_action item={} result={reply:?}", item.id);
        reply
    }

    fn act(
        &self,
        (id, action): (&str, crate::Action),
        deadline: Instant,
        await_start: impl FnOnce(Instant) -> Result<(), Reply>,
    ) -> Reply {
        let paths = self.ready.paths();
        if action != crate::Action::Stop && !hosting::available(paths) {
            return refused(Refusal::Busy);
        }
        let store = match container::store(paths, action) {
            Ok(store) => store,
            Err(Fault::Storage(io::ErrorKind::WouldBlock)) => return refused(Refusal::Busy),
            Err(fault) => return failed(fault),
        };
        if let Err(reply) = self.plan(&store, (id, action), deadline) {
            return reply;
        }
        if !self.enabled() {
            return refused(Refusal::ApplicationsOff);
        }
        if Instant::now() >= deadline {
            return failed(Fault::LimitSeconds);
        }
        if let Err(reply) = await_start(deadline) {
            return reply;
        }
        let plan = match self.plan(&store, (id, action), deadline) {
            Ok(plan) => plan,
            Err(Reply::Refused { cause }) if cause == Refusal::MigrationHeld.code() => {
                return failed(Fault::MigrationHeld);
            }
            Err(Reply::Refused { .. }) => return failed(Fault::CommandFailed),
            Err(reply) => return reply,
        };
        run_container(paths, &plan, action, deadline).map_or_else(failed, |()| Reply::Succeeded)
    }

    fn plan(
        &self,
        store: &Store<'_>,
        (id, action): (&str, crate::Action),
        deadline: Instant,
    ) -> Result<Plan, Reply> {
        let paths = self.ready.paths();
        let (inventory, records) = container::observe(store, paths, (id, action), deadline)
            .map_err(|fault| {
                if fault == Fault::MigrationHeld {
                    refused(Refusal::MigrationHeld)
                } else {
                    failed(fault)
                }
            })?;
        action_plan(
            id,
            action,
            (self.ready.settings(), paths),
            (&inventory, &records),
        )
        .map_err(refused)
    }
}

fn refused(cause: Refusal) -> Reply {
    Reply::Refused {
        cause: cause.code().to_owned(),
    }
}

fn start_failed(kind: io::ErrorKind) -> Reply {
    if matches!(kind, io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock) {
        failed(Fault::LimitSeconds)
    } else {
        failed(Fault::CommandFailed)
    }
}

fn failed(fault: Fault) -> Reply {
    let cause = match fault {
        Fault::LimitSeconds => "limit_seconds",
        Fault::VersionUnconfirmed | Fault::HostingUnconfirmed => "orphan",
        Fault::MigrationHeld => "migration_held",
        Fault::HostingBusy => "busy",
        _ => "step_failed",
    };
    Reply::Failed {
        cause: cause.to_owned(),
    }
}

struct Slot<'a>(&'a AtomicUsize);

impl<'a> Slot<'a> {
    fn claim(active: &'a AtomicUsize) -> Option<Self> {
        active
            .try_update(Ordering::Relaxed, Ordering::Relaxed, |used| {
                (used < CONNECTIONS).then_some(used + 1)
            })
            .ok()
            .map(|_| Self(active))
    }
}

impl Drop for Slot<'_> {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::Relaxed);
    }
}
