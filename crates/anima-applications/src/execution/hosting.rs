//! Admit start-capable Docker commands on the installed system.
use std::time::Instant;

use anima_os_maintenance::{
    cutover::APPLICATION_ROOT,
    hosting::{self, Dispatch, Operation},
};

use crate::{Command, Paths};

use super::{Fault, process};

pub(crate) fn available(paths: &Paths) -> bool {
    paths.root != APPLICATION_ROOT || hosting::available().is_ok()
}

pub(super) fn run(
    paths: &Paths,
    operation: Operation,
    command: &Command,
    deadline: Instant,
) -> Result<(), Fault> {
    if paths.root != APPLICATION_ROOT {
        return process::run(paths, command, deadline);
    }
    let dispatch = Dispatch::begin(operation).map_err(|error| {
        eprintln!("hosting dispatch: {error}");
        Fault::HostingBusy
    })?;
    let result = process::run(paths, command, deadline);
    if !matches!(result, Ok(()) | Err(Fault::UnsafePlan | Fault::Spawn(_))) {
        return Err(Fault::HostingUnconfirmed);
    }
    dispatch.finished().map_err(|error| {
        eprintln!("hosting dispatch: {error}");
        Fault::HostingUnconfirmed
    })?;
    result
}
