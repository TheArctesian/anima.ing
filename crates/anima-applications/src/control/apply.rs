//! Run a managed Compose apply on the authenticated owner socket.
mod cutover;
mod health;
mod images;
pub(in crate::control) mod observe;
pub(in crate::control) mod records;

pub(crate) use cutover::verify_candidate;

use std::{os::unix::net::UnixStream, time::Instant};

use anima_applications_wire::Reply;

use crate::{
    Apply, Inventory, PairFile, Records, Refusal,
    execution::{Executor, Fault, hosting},
    forms::project_of,
    settings::Machine,
    startup::Ready,
    work::{
        Facts,
        apply::{self, Files, Prepared},
    },
};

use super::{failed, refused, start_failed, wire};

pub(super) fn run(
    (ready, apply): (&Ready, &Apply),
    stream: &mut UnixStream,
    deadline: Instant,
    authorized: impl Fn() -> bool,
) -> Reply {
    if !hosting::available(ready.paths()) {
        return refused(Refusal::Busy);
    }
    let mut executor = match ready.executor() {
        Ok(executor) => executor,
        Err(Fault::Storage(std::io::ErrorKind::WouldBlock)) => return refused(Refusal::Busy),
        Err(fault) => return failed(fault),
    };
    let folders = ready.paths().application(&apply.application);
    if crate::migration::check_start(
        executor.store(),
        (&folders, &ready.paths().socket),
        deadline,
    )
    .is_err()
    {
        return refused(Refusal::MigrationHeld);
    }
    let project = project_of(&apply.application);
    let inputs = match Inputs::read(ready, &executor, (apply, &project), deadline) {
        Ok(inputs) => inputs,
        Err(fault) => return failed(fault),
    };
    let facts = inputs.facts(ready);
    if let Err(cause) = apply::accepted(apply, &facts) {
        return refused(cause);
    }
    if !authorized() {
        return refused(Refusal::ApplicationsOff);
    }
    let prepared = match stage(stream, apply, &facts, deadline) {
        Ok(prepared) => prepared,
        Err(reply) => return reply,
    };
    let permitted = || {
        if authorized() && wire::connected(stream) {
            Ok(())
        } else {
            Err(Fault::CommandFailed)
        }
    };
    let result = execute(
        (ready, &mut executor),
        (&prepared, &project),
        inputs,
        (deadline, permitted),
    );
    drop(executor);
    if let Err(failure) = result {
        return failure.reply();
    }
    let folder = ready.paths().application(&apply.application).files();
    health::report(
        (ready.paths(), stream),
        (prepared, &project, &folder),
        deadline,
        authorized,
    )
}

struct Inputs {
    machine: Machine,
    records: Records,
    inventory: Inventory,
}

impl Inputs {
    fn read(
        ready: &Ready,
        executor: &Executor<'_>,
        (apply, project): (&Apply, &str),
        deadline: Instant,
    ) -> Result<Self, Fault> {
        let paths = ready.paths();
        Ok(Self {
            machine: observe::machine(paths, deadline)?,
            records: records::read(executor.store(), paths, &apply.application)?,
            inventory: observe::inventory(paths, project, deadline)?,
        })
    }

    fn facts<'a>(&'a self, ready: &'a Ready) -> Facts<'a> {
        Facts {
            settings: ready.settings(),
            paths: ready.paths(),
            inventory: &self.inventory,
            records: &self.records,
            pair: PairFile::Missing,
            machine: self.machine,
            busy: false,
        }
    }
}

fn stage(
    stream: &mut UnixStream,
    apply: &Apply,
    facts: &Facts<'_>,
    deadline: Instant,
) -> Result<Prepared, Reply> {
    wire::send(stream, &Reply::NeedsFiles).map_err(|error| start_failed(error.kind()))?;
    let received = wire::receive_files(stream, apply.environment_sha256.is_some(), deadline)
        .map_err(|error| start_failed(error.kind()))?;
    let files = Files {
        compose: &received.compose,
        environment: received.environment.as_ref().map(|file| file.as_slice()),
    };
    let prepared =
        apply::prepared(apply, &files, facts).map_err(|cause| Failure::Check(cause).reply())?;
    drop(received);
    wire::send(stream, &Reply::Ready).map_err(|error| start_failed(error.kind()))?;
    wire::await_start(stream, deadline).map_err(|error| start_failed(error.kind()))?;
    Ok(prepared)
}

fn execute(
    (ready, executor): (&Ready, &mut Executor<'_>),
    (prepared, project): (&Prepared, &str),
    mut inputs: Inputs,
    (deadline, permitted): (Instant, impl Fn() -> Result<(), Fault>),
) -> Result<(), Failure> {
    let paths = ready.paths();
    let mut digests = Vec::with_capacity(prepared.pulls.len());
    for request in &prepared.pulls {
        permitted()?;
        if observe::machine(paths, deadline)?.free_disk_bytes
            < ready.settings().limits.free_disk_bytes
        {
            return Err(Refusal::LimitDisk.into());
        }
        permitted()?;
        let pulled = images::pull(paths, request, deadline)?;
        eprintln!(
            "application_apply project={project} image_digest={}",
            pulled.digest
        );
        digests.push(pulled);
    }
    permitted()?;
    inputs.inventory = observe::inventory(paths, project, deadline)?;
    let plan = apply::planned(prepared, &digests, &inputs.facts(ready))?;
    eprintln!(
        "application_apply project={project} final_form_sha256={}",
        plan.final_form_sha256
    );
    let report = executor.run_while(&plan.plan, deadline, permitted);
    if let Some(failure) = report.failed {
        return Err(failure.fault.into());
    }
    Ok(())
}

enum Failure {
    Fault(Fault),
    Check(Refusal),
}

impl Failure {
    fn reply(self) -> Reply {
        match self {
            Self::Fault(fault) => failed(fault),
            Self::Check(cause) => Reply::Failed {
                cause: cause.code().to_owned(),
            },
        }
    }
}

impl From<Fault> for Failure {
    fn from(fault: Fault) -> Self {
        Self::Fault(fault)
    }
}

impl From<Refusal> for Failure {
    fn from(cause: Refusal) -> Self {
        Self::Check(cause)
    }
}
