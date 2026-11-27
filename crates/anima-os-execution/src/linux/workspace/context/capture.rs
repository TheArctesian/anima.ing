//! Snapshot accepted source under admission, then copy the immutable tree in isolation.
use super::super::checkout;
use super::{
    Context, Error, Identity, QUOTA, ROOT, Result, command, path, read, record, save, tree,
};
use anima_os_maintenance::{Admission, source_capture::Activity, source_pipeline::Pipeline};
use serde_json::Value;
use std::{
    fs::{self, File},
    os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt},
    path::{Path, PathBuf},
    sync::Arc,
};
use uuid::Uuid;

pub async fn capture(identity: Identity, pipeline: Option<Pipeline>) -> Result<Value> {
    let Identity {
        workspace,
        generation,
        context: id,
    } = identity;
    if id.is_nil() {
        return Err(Error::Refused("Choose a valid source context identity."));
    }
    if pipeline.is_some_and(|owner| owner.workspace() != workspace || generation != 1) {
        return Err(Error::Refused(
            "Source capture does not match the admitted pipeline.",
        ));
    }
    let pending = {
        let admission = Admission::for_pipeline(pipeline)?;
        let allocation = record::allocation()?;
        let _workspace = record::lock(workspace)?;
        let source = require_stable(workspace, generation)?;
        let base = Path::new(ROOT).join(id.to_string());
        if base.try_exists()? {
            return previous(&base.join("context.json"), identity);
        }
        prepare_capture(&admission, &allocation).await?;
        Pending::snapshot(&admission, identity, source, base).await?
    };
    pending.finish().await
}

struct Guards {
    _context: File,
    _activity: Activity,
}

pub(super) fn lock_context(id: Uuid) -> Result<File> {
    let lock = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .custom_flags(nix::libc::O_NOFOLLOW)
        .open(record::locks().join(format!("context-{id}")))?;
    lock.try_lock().map_err(|error| match error {
        fs::TryLockError::WouldBlock => {
            Error::Refused("A source capture is in progress for this context.")
        }
        fs::TryLockError::Error(error) => Error::Io(error),
    })?;
    Ok(lock)
}

struct Pending {
    base: PathBuf,
    context: Context,
    guards: Arc<Guards>,
}

impl Pending {
    async fn snapshot(
        admission: &Admission,
        identity: Identity,
        source: Option<checkout::Source>,
        base: PathBuf,
    ) -> Result<Self> {
        let guards = Arc::new(Guards {
            _context: lock_context(identity.context)?,
            _activity: Activity::begin(admission)?,
        });
        if !Path::new(ROOT).try_exists()? {
            fs::DirBuilder::new().mode(0o700).create(ROOT)?;
        }
        fs::DirBuilder::new().mode(0o700).create(&base)?;
        let context = Context {
            schema: 1,
            source,
            id: identity.context,
            workspace: identity.workspace,
            generation: identity.generation,
            phase: "capturing".into(),
            digest: None,
            bytes: 0,
            entries: 0,
            quota_bytes: QUOTA,
        };
        save(&base.join("context.json"), &context)?;
        let home = record::home(identity.workspace);
        let snapshot = base.join("snapshot");
        command::run(
            "btrfs",
            &[
                "subvolume",
                "snapshot",
                "-r",
                path(&home)?,
                path(&snapshot)?,
            ],
        )
        .await?;
        Ok(Self {
            base,
            context,
            guards,
        })
    }

    async fn finish(mut self) -> Result<Value> {
        let snapshot = self.base.join("snapshot");
        let export = self.base.join("source");
        let outcome = export_source(&snapshot, &export, Arc::clone(&self.guards)).await;
        // Only the filtered export is a build input.
        command::run("btrfs", &["subvolume", "delete", path(&snapshot)?]).await?;
        match outcome {
            Ok(tree) => {
                self.context.phase = "captured".into();
                self.context.bytes = tree.bytes;
                self.context.entries = tree.entries;
                self.context.digest = Some(tree.digest());
            }
            Err(_) => self.context.phase = "interrupted".into(),
        }
        save(&self.base.join("context.json"), &self.context)?;
        Ok(serde_json::to_value(self.context)?)
    }
}

async fn export_source(snapshot: &Path, export: &Path, guards: Arc<Guards>) -> Result<tree::Tree> {
    command::run("btrfs", &["subvolume", "create", path(export)?]).await?;
    command::run(
        "btrfs",
        &["qgroup", "limit", &QUOTA.to_string(), path(export)?],
    )
    .await?;
    let tree = copy_tree(snapshot.join("workspace"), export.to_owned(), guards).await?;
    fs::set_permissions(export, fs::Permissions::from_mode(0o755))?;
    command::run(
        "btrfs",
        &["property", "set", "-ts", path(export)?, "ro", "true"],
    )
    .await?;
    Ok(tree)
}

async fn copy_tree(
    source: PathBuf,
    destination: PathBuf,
    guards: Arc<Guards>,
) -> Result<tree::Tree> {
    let (send, receive) = tokio::sync::oneshot::channel();
    // A fresh thread inherits the priorities set before the snapshot, even if
    // the runtime already has blocking threads running at a different priority.
    std::thread::Builder::new()
        .name("anima-source-copy".into())
        .spawn(move || {
            let _guards = guards;
            drop(send.send(tree::Tree::capture(&source, &destination)));
        })?;
    receive
        .await
        .map_err(|_| Error::Refused("Source copying did not complete."))?
}

async fn prepare_capture(admission: &Admission, allocation: &record::Allocation) -> Result<()> {
    admission.require_open_work()?;
    if record::free_storage(allocation)? < QUOTA {
        return Err(Error::Refused(
            "Hosting reserves the remaining source capture capacity.",
        ));
    }
    let pid = std::process::id().to_string();
    command::run("ionice", &["-c", "3", "-p", &pid]).await?;
    command::run("renice", &["--priority", "19", "--pid", &pid]).await?;
    Ok(admission.require_open_work()?)
}

fn require_stable(workspace: Uuid, generation: u64) -> Result<Option<checkout::Source>> {
    let state = record::required(workspace)?;
    if state.generation != generation
        || matches!(
            state.phase,
            record::Phase::Preparing
                | record::Phase::Starting
                | record::Phase::Stopping
                | record::Phase::Unknown
                | record::Phase::Deleting
                | record::Phase::Deleted
        )
    {
        return Err(Error::Refused(
            "Reconcile the workspace before capturing its source.",
        ));
    }
    state
        .checkout
        .as_ref()
        .map(checkout::Receipt::captured_source)
        .transpose()
}

fn previous(receipt: &Path, identity: Identity) -> Result<Value> {
    let Identity {
        workspace,
        generation,
        context: id,
    } = identity;
    let previous: Context = read(receipt)?;
    if previous.schema != 1
        || previous.id != id
        || previous.workspace != workspace
        || previous.generation != generation
    {
        return Err(Error::Refused(
            "Source context identity belongs to another request.",
        ));
    }
    Ok(serde_json::to_value(previous)?)
}
