//! Durable workspace removal after every runtime and captured-source owner releases it.
use super::{
    Error, Result, command, context,
    record::{self, Phase},
};
use crate::linux::builds;
use agent_threads::managed::workspace::handoff::Source;
use anima_os_maintenance::{Admission, reservations};
use serde_json::Value;
use std::{fs, path::Path};
use uuid::Uuid;

pub async fn release(id: Uuid, generation: u64, confirmed: bool) -> Result<Value> {
    if !confirmed {
        return Err(Error::Refused(
            "Confirm deletion of this workspace and its files.",
        ));
    }
    let admission = Admission::cleanup()?;
    let _allocation = record::allocation()?;
    let _workspace = record::lock(id)?;
    let state = record::required(id)?;
    if state.generation != generation {
        return Err(Error::Refused("The workspace generation changed."));
    }
    if state.handoff.is_some() {
        return Err(Error::Refused(
            "This workspace retains the source for a handoff.",
        ));
    }
    if !matches!(
        state.phase,
        Phase::Prepared | Phase::Stopped | Phase::Deleting | Phase::Deleted
    ) {
        return Err(Error::Refused(
            "Stop and reconcile this workspace before deleting it.",
        ));
    }
    Ok(serde_json::to_value(
        release_locked(state, &admission).await?,
    )?)
}

pub(in crate::linux) async fn retire(source: Source) -> Result<Value> {
    let admission = Admission::cleanup()?;
    let _allocation = record::allocation()?;
    let _workspace = record::lock(source.workspace)?;
    let state = record::required(source.workspace)?;
    require_retained_source(&state, source)?;
    release_locked(state, &admission).await?;
    Ok(serde_json::json!({"source":source,"state":"retired"}))
}

fn require_retained_source(state: &record::Record, source: Source) -> Result<()> {
    if !source.fits()
        || state.workspace != source.workspace
        || state.generation != source.generation
        || state.principal != Some(source.principal)
        || state.handoff != Some(source.binding)
        || !matches!(
            state.phase,
            Phase::Stopped | Phase::Deleting | Phase::Deleted
        )
    {
        return Err(Error::Refused(
            "This request does not own the retained source.",
        ));
    }
    Ok(())
}

async fn release_locked(
    mut state: record::Record,
    admission: &Admission,
) -> Result<record::Record> {
    let (id, generation) = (state.workspace, state.generation);
    context::require_released(id)?;
    builds::store::require_released(id)?;
    absent_runtime(id).await?;
    if let Some(run) = state.run {
        reservations::release_stopped(admission, run, generation).await?;
    }
    let home = record::home(id);
    if state.phase != Phase::Deleted {
        state.phase = Phase::Deleting;
        record::save(&state)?;
        remove_home(&home).await?;
    }
    match fs::symlink_metadata(&home) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
        _ => {
            return Err(Error::Refused(
                "Workspace storage removal has not completed.",
            ));
        }
    }
    fs::File::open(
        home.parent()
            .ok_or(Error::Refused("Invalid workspace parent."))?,
    )?
    .sync_all()?;
    state.phase = Phase::Deleted;
    state.storage_bytes = 0;
    record::save(&state)?;
    Ok(state)
}

pub(super) async fn remove_home(home: &Path) -> Result<()> {
    match fs::symlink_metadata(home) {
        Ok(metadata) if metadata.is_dir() => {
            let path = home
                .to_str()
                .ok_or(Error::Refused("Invalid workspace storage path."))?;
            command::run("btrfs", &["subvolume", "delete", path]).await?;
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
        _ => {
            return Err(Error::Refused(
                "Workspace storage is not its recorded subvolume.",
            ));
        }
    }
    Ok(())
}

pub(super) async fn absent_runtime(id: Uuid) -> Result<()> {
    let label = format!("label=ing.anima.workspace={id}");
    let container = format!("name=^anima-workspace-{id}-");
    let network = format!("name=^anima-workspace-{id}$");
    for filter in [&label, &container] {
        if !command::docker(&["ps", "--all", "--quiet", "--filter", filter])
            .await?
            .trim()
            .is_empty()
        {
            return Err(Error::Refused(
                "A container still references this workspace.",
            ));
        }
    }
    for filter in [&label, &network] {
        if !command::docker(&["network", "ls", "--quiet", "--filter", filter])
            .await?
            .trim()
            .is_empty()
        {
            return Err(Error::Refused("A network still references this workspace."));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{Phase, Source, record::Record, require_retained_source};
    use agent_threads::managed::workspace::handoff::Binding;
    use uuid::Uuid;

    #[test]
    fn retained_source_cleanup_is_bound_even_when_replaying_a_deleted_record() {
        let source = Source {
            workspace: Uuid::from_u128(1),
            principal: Uuid::from_u128(2),
            generation: 7,
            binding: Binding {
                transfer: Uuid::from_u128(3),
                destination: Uuid::from_u128(4),
            },
        };
        let mut state = Record::new(source.workspace, 1 << 30);
        state.principal = Some(source.principal);
        state.generation = source.generation;
        state.handoff = Some(source.binding);
        state.run = Some(Uuid::from_u128(6));
        state.phase = Phase::Stopping;
        assert!(require_retained_source(&state, source).is_err());
        state.phase = Phase::Stopped;
        assert!(require_retained_source(&state, source).is_ok());
        state.phase = Phase::Deleting;
        assert!(require_retained_source(&state, source).is_ok());
        state.phase = Phase::Deleted;
        state.storage_bytes = 0;
        let saved = serde_json::to_vec(&state).unwrap();
        let replay: Record = serde_json::from_slice(&saved).unwrap();
        assert!(super::super::handoff::validate_record(source.binding, &replay).is_ok());
        assert!(require_retained_source(&replay, source).is_ok());
        assert!(
            require_retained_source(
                &replay,
                Source {
                    generation: 6,
                    ..source
                }
            )
            .is_err()
        );
        assert!(
            require_retained_source(
                &replay,
                Source {
                    binding: Binding {
                        destination: Uuid::from_u128(5),
                        ..source.binding
                    },
                    ..source
                }
            )
            .is_err()
        );
    }
}
