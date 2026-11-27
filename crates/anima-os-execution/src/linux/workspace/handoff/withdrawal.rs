//! A durable refusal of stale transfer offers, written under the workspace lock.
use super::{Error, Result, authorize, record};
use agent_threads::managed::workspace::{Phase, View, handoff::Source};
use anima_os_maintenance::Admission;
use serde_json::{Value, json};
use std::{
    fs::{self, File, OpenOptions},
    io::{ErrorKind, Read, Write},
    os::unix::fs::OpenOptionsExt as _,
    path::{Path, PathBuf},
};

#[derive(Clone, Copy)]
pub(super) enum Side {
    Source,
    Destination,
}

pub(super) async fn withdraw(source: Source) -> Result<Value> {
    if !source.fits() {
        return Err(Error::Refused("The source withdrawal is invalid."));
    }
    let admission = Admission::cleanup()?;
    let _lock = record::lock(source.workspace)?;
    let mut state = record::required(source.workspace)?;
    authorize(&state, source.generation, source.principal)?;
    if let Some(binding) = state.handoff {
        if binding != source.binding {
            return Err(Error::Refused("Another transfer holds this source."));
        }
        state = super::super::stop_locked(state, &admission).await?;
    }
    seal(source, Side::Source)?;
    if state.handoff.take().is_some() {
        record::save(&state)?;
    } else {
        record::sync()?;
    }
    let stopped = (state.phase == record::Phase::Stopped).then(|| View {
        workspace: state.workspace,
        generation: state.generation,
        phase: Phase::Stopped,
        storage_bytes: state.storage_bytes,
        checkout: state
            .checkout
            .as_ref()
            .map(super::super::checkout::Receipt::view),
    });
    Ok(json!({"source":source,"state":"withdrawn","stopped":stopped}))
}

pub(super) fn require_open(source: Source, side: Side) -> Result<()> {
    if sealed(&path(source, side), source)? {
        return Err(Error::Refused("This workspace transfer was withdrawn."));
    }
    Ok(())
}

fn sealed(path: &Path, source: Source) -> Result<bool> {
    let file = match OpenOptions::new()
        .read(true)
        .custom_flags(nix::libc::O_NOFOLLOW)
        .open(path)
    {
        Ok(file) => file,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error.into()),
    };
    let mut bytes = Vec::new();
    file.take(513).read_to_end(&mut bytes)?;
    if bytes.len() > 512 || serde_json::from_slice::<Source>(&bytes)? != source {
        return Err(Error::Refused(
            "The withdrawal marker does not match this transfer.",
        ));
    }
    Ok(true)
}

pub(super) fn seal(source: Source, side: Side) -> Result<()> {
    publish(&path(source, side), source)
}

fn publish(path: &Path, source: Source) -> Result<()> {
    let parent = path
        .parent()
        .ok_or(Error::Refused("Workspace locks are unavailable."))?;
    if !sealed(path, source)? {
        let pending = path.with_extension("pending");
        let mut file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .custom_flags(nix::libc::O_NOFOLLOW)
            .open(&pending)?;
        file.write_all(&serde_json::to_vec(&source)?)?;
        file.sync_all()?;
        fs::rename(pending, path)?;
    }
    File::open(parent)?.sync_all()?;
    Ok(())
}

fn path(source: Source, side: Side) -> PathBuf {
    let side = match side {
        Side::Source => "source",
        Side::Destination => "destination",
    };
    record::locks().join(format!(
        "withdraw-{side}-{}-{}",
        source.workspace, source.binding.transfer,
    ))
}

#[cfg(test)]
mod tests {
    use super::{publish, sealed};
    use agent_threads::managed::workspace::handoff::{Binding, Source};
    use std::fs;
    use uuid::Uuid;

    #[test]
    fn sealed_transfer_is_durable_and_can_only_acknowledge_the_same_binding() {
        let scratch = std::env::temp_dir().join(format!("anima-withdrawal-{}", Uuid::now_v7()));
        fs::create_dir(&scratch).unwrap();
        let marker = scratch.join("transfer");
        let source = Source {
            workspace: Uuid::from_u128(1),
            principal: Uuid::from_u128(2),
            generation: 7,
            binding: Binding {
                transfer: Uuid::from_u128(3),
                destination: Uuid::from_u128(4),
            },
        };
        assert!(!sealed(&marker, source).unwrap());
        publish(&marker, source).unwrap();
        publish(&marker, source).unwrap();
        assert!(sealed(&marker, source).unwrap());
        let wrong = Source {
            generation: 8,
            ..source
        };
        assert!(publish(&marker, wrong).is_err());
        assert!(sealed(&marker, source).unwrap());
        fs::remove_dir_all(scratch).unwrap();
    }
}
