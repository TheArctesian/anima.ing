//! An exact incoming handoff owns its quota but cannot run until activation.
mod archive;
mod stream;
mod tree;

use super::{
    project,
    withdrawal::{self, Side},
};
use crate::linux::{
    Error, Result,
    workspace::{
        checkout, cleanup,
        record::{self, Phase, Record},
        recovery,
    },
};
use agent_threads::managed::workspace::handoff::Import;
use anima_os_maintenance::Admission;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    fs::{self, File},
    io,
    time::Duration,
};
use tokio::io::AsyncRead;

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(in crate::linux::workspace) struct Incoming {
    import: Import,
    stage: Stage,
}

#[derive(Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
enum Stage {
    Restoring,
    Restored,
    Activated,
}

impl Incoming {
    pub(in crate::linux::workspace) fn activated(&self) -> bool {
        self.stage == Stage::Activated
    }

    pub(in crate::linux::workspace) fn validate(&self, state: &Record) -> Result<()> {
        let source = self.import.receipt.source;
        if !self.import.fits()
            || source.workspace != state.workspace
            || state.principal != Some(source.principal)
            || (state.phase != Phase::Deleted && state.storage_bytes != self.import.storage_bytes)
        {
            return Err(Error::Refused(
                "The incoming workspace receipt needs reconciliation.",
            ));
        }
        let checkout = state.checkout.as_ref().map(checkout::Receipt::view);
        if checkout.as_ref().map(|record| &record.source) != self.import.checkout.as_ref()
            || checkout.is_some_and(|record| !record.ready)
        {
            return Err(Error::Refused(
                "The imported checkout provenance needs reconciliation.",
            ));
        }
        let valid = if self.activated() {
            valid_activation(state, source.generation)
        } else {
            state.phase == Phase::Preparing
                && state.generation == source.generation
                && state.run.is_none()
                && state.handoff.is_none()
        };
        if !valid {
            return Err(Error::Refused(
                "The incoming workspace phase needs reconciliation.",
            ));
        }
        Ok(())
    }
}

fn valid_activation(state: &Record, imported_generation: u64) -> bool {
    if state.generation == imported_generation {
        return state.run.is_none()
            && matches!(
                state.phase,
                Phase::Prepared | Phase::Deleting | Phase::Deleted
            );
    }
    state.generation > imported_generation && state.phase != Phase::Preparing
}

pub(in crate::linux) async fn receive(import: Import, input: impl AsyncRead + Unpin) -> Result<()> {
    let checkpoint = project::checkpoint_lock()?;
    let (workspace, mut state) = reserve(import)?;
    if current(&state)?.stage != Stage::Restoring {
        return crate::linux::reply(view(&state)).await;
    }
    let home = record::home(state.workspace);
    reset_home(&state, &home).await?;
    let receipt = current(&state)?.import.receipt.clone();
    let locks = (checkpoint.try_clone()?, workspace.try_clone()?);
    crate::linux::reply(Ok(json!({"stream":"workspace-handoff-project-restore/v1"}))).await?;
    stream::unpack(input, home.clone(), (receipt, state.storage_bytes), locks).await?;
    File::open(&home)?.sync_all()?;
    File::open(
        home.parent()
            .ok_or(Error::Refused("Workspace parent unavailable."))?,
    )?
    .sync_all()?;
    set_stage(&mut state, Stage::Restored)?;
    record::save(&state)?;
    crate::linux::reply(view(&state)).await
}

fn reserve(import: Import) -> Result<(File, Record)> {
    if !import.fits() {
        return Err(Error::Refused(
            "The destination checkpoint request is invalid.",
        ));
    }
    let source = import.receipt.source;
    let admission = Admission::acquire()?;
    let allocation = record::allocation()?;
    let lock = record::lock(source.workspace)?;
    withdrawal::require_open(source, Side::Destination)?;
    if let Some(existing) = record::read(source.workspace)?
        && !returning_to_retired_source(&existing, &import)
    {
        return replay(lock, existing, &import, &admission);
    }
    anima_os_maintenance::recovery::require_ready()?;
    admission.require_open_work()?;
    if !super::super::has_room()? || record::free_storage(&allocation)? < import.storage_bytes {
        return Err(Error::Refused(
            "Hosting reserves the remaining destination workspace capacity.",
        ));
    }
    match fs::symlink_metadata(record::home(source.workspace)) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => (),
        Err(error) => return Err(error.into()),
        Ok(_) => return Err(Error::Refused("Destination storage already exists.")),
    }
    let mut state = Record::new(source.workspace, import.storage_bytes);
    state.principal = Some(source.principal);
    state.generation = source.generation;
    if let Some(repository) = import.checkout.clone() {
        let repository = checkout::Source::from(repository);
        repository.authorize()?;
        state.checkout = Some(checkout::Receipt::restored(repository));
    }
    state.incoming = Some(Incoming {
        import,
        stage: Stage::Restoring,
    });
    record::save(&state)?;
    Ok((lock, state))
}

fn returning_to_retired_source(existing: &Record, import: &Import) -> bool {
    let source = import.receipt.source;
    existing.phase == Phase::Deleted
        && existing.storage_bytes == 0
        && existing.handoff.is_some()
        && existing.principal == Some(source.principal)
        && existing.generation < source.generation
}

fn replay(
    lock: File,
    existing: Record,
    import: &Import,
    admission: &Admission,
) -> Result<(File, Record)> {
    if matching(&existing, import)?.stage == Stage::Restoring {
        anima_os_maintenance::recovery::require_ready()?;
        admission.require_open_work()?;
    }
    Ok((lock, existing))
}

async fn reset_home(state: &Record, home: &std::path::Path) -> Result<()> {
    tokio::time::timeout(
        Duration::from_secs(10),
        cleanup::absent_runtime(state.workspace),
    )
    .await
    .map_err(|_| Error::Refused("The destination runtime could not be checked."))??;
    cleanup::remove_home(home).await?;
    recovery::make_home(state).await
}

pub(in crate::linux) fn status(import: &Import) -> Result<Value> {
    let state = record::required(import.receipt.source.workspace)?;
    matching(&state, import)?;
    view(&state)
}

pub(in crate::linux) fn activate(import: &Import) -> Result<Value> {
    let workspace = import.receipt.source.workspace;
    let admission = Admission::acquire()?;
    let _lock = record::lock(workspace)?;
    withdrawal::require_open(import.receipt.source, Side::Destination)?;
    let mut state = record::required(workspace)?;
    match matching(&state, import)?.stage {
        Stage::Restoring => {
            return Err(Error::Refused(
                "Finish and verify the checkpoint before activation.",
            ));
        }
        Stage::Activated => return view(&state),
        Stage::Restored => (),
    }
    anima_os_maintenance::recovery::require_ready()?;
    admission.require_open_work()?;
    if let Some(checkout) = &state.checkout {
        checkout.source().authorize()?;
    }
    set_stage(&mut state, Stage::Activated)?;
    state.phase = Phase::Prepared;
    record::save(&state)?;
    view(&state)
}

pub(in crate::linux) async fn discard(import: &Import) -> Result<Value> {
    if !import.fits() {
        return Err(Error::Refused("The destination withdrawal is invalid."));
    }
    let source = import.receipt.source;
    let _admission = Admission::cleanup()?;
    let _allocation = record::allocation()?;
    let _lock = record::lock(source.workspace)?;
    let state = record::read(source.workspace)?
        .filter(|existing| !returning_to_retired_source(existing, import));
    if let Some(state) = &state
        && matching(state, import)?.activated()
    {
        return view(state);
    }
    withdrawal::seal(source, Side::Destination)?;
    if state.is_some() {
        cleanup::absent_runtime(source.workspace).await?;
        let home = record::home(source.workspace);
        cleanup::remove_home(&home).await?;
        File::open(
            home.parent()
                .ok_or(Error::Refused("Workspace parent unavailable."))?,
        )?
        .sync_all()?;
        record::remove_import(source.workspace)?;
    } else {
        record::sync()?;
    }
    Ok(json!({"import":import,"state":"discarded"}))
}

fn matching<'a>(state: &'a Record, import: &Import) -> Result<&'a Incoming> {
    let saved = current(state)?;
    if &saved.import != import {
        return Err(Error::Refused(
            "The destination workspace belongs to another request.",
        ));
    }
    Ok(saved)
}

fn current(state: &Record) -> Result<&Incoming> {
    state
        .incoming
        .as_ref()
        .ok_or(Error::Refused("No incoming handoff owns this workspace."))
}

fn set_stage(state: &mut Record, next: Stage) -> Result<()> {
    state
        .incoming
        .as_mut()
        .ok_or(Error::Refused("No incoming handoff owns this workspace."))?
        .stage = next;
    Ok(())
}

fn view(state: &Record) -> Result<Value> {
    let saved = current(state)?;
    Ok(
        json!({"workspace":state.workspace,"generation":state.generation,
        "transfer":saved.import.receipt.source.binding.transfer,"digest":saved.import.receipt.digest,
        "state":saved.stage, "workspacePhase":state.phase}),
    )
}

#[cfg(test)]
mod tests;
