use super::{Context, Error, QUOTA, ROOT, Result, capture, command, path, read, record, save};
use serde_json::Value;
use std::{fs, path::Path};
use uuid::Uuid;

pub fn status(workspace: Uuid, generation: u64, id: Uuid) -> Result<Value> {
    let _allocation = record::allocation()?;
    let context = owned(workspace, generation, id)?;
    Ok(serde_json::to_value(context)?)
}

pub async fn release(workspace: Uuid, generation: u64, id: Uuid, confirmed: bool) -> Result<Value> {
    if !confirmed {
        return Err(Error::Refused(
            "Confirm deletion of this captured source context.",
        ));
    }
    let _admission = anima_os_maintenance::Admission::cleanup()?;
    let _allocation = record::allocation()?;
    let _context = capture::lock_context(id)?;
    let mut context = owned(workspace, generation, id)?;
    let base = Path::new(ROOT).join(id.to_string());
    let leases = base.join("leases");
    if leases.try_exists()? && fs::read_dir(&leases)?.next().is_some() {
        return Err(Error::Refused(
            "A build still holds this source context. Reconcile that attempt first.",
        ));
    }
    let receipt = base.join("context.json");
    if context.phase == "deleted" {
        ensure_absent(&base)?;
        return Ok(serde_json::to_value(context)?);
    }
    context.phase = "deleting".into();
    save(&receipt, &context)?;
    for name in ["snapshot", "source"] {
        let target = base.join(name);
        if target.try_exists()? {
            if !fs::symlink_metadata(&target)?.is_dir() {
                return Err(Error::Refused(
                    "Captured source storage needs reconciliation.",
                ));
            }
            command::run("btrfs", &["subvolume", "delete", path(&target)?]).await?;
        }
    }
    ensure_absent(&base)?;
    context.phase = "deleted".into();
    context.quota_bytes = 0;
    save(&receipt, &context)?;
    Ok(serde_json::to_value(context)?)
}

fn ensure_absent(base: &Path) -> Result<()> {
    for name in ["snapshot", "source"] {
        if fs::symlink_metadata(base.join(name)).is_ok() {
            return Err(Error::Refused("Captured source removal has not completed."));
        }
        // Access failures cannot be interpreted as proof that storage is absent.
        if base.join(name).try_exists()? {
            return Err(Error::Refused("Captured source removal has not completed."));
        }
    }
    Ok(())
}

fn owned(workspace: Uuid, generation: u64, id: Uuid) -> Result<Context> {
    if id.is_nil() {
        return Err(Error::Refused("Choose a valid source context identity."));
    }
    let context: Context = read(&Path::new(ROOT).join(id.to_string()).join("context.json"))?;
    if context.schema != 1
        || context.id != id
        || context.workspace != workspace
        || context.generation != generation
        || (context.quota_bytes != QUOTA
            && !(context.phase == "deleted" && context.quota_bytes == 0))
    {
        return Err(Error::Refused(
            "Source context ownership or quota does not match.",
        ));
    }
    Ok(context)
}

pub(in crate::linux::workspace) fn require_released(workspace: Uuid) -> Result<()> {
    if !Path::new(ROOT).try_exists()? {
        return Ok(());
    }
    for (count, entry) in fs::read_dir(ROOT)?.enumerate() {
        if count >= 16384 {
            return Err(Error::Refused(
                "Source context inventory exceeded its bound.",
            ));
        }
        let entry = entry?;
        let id = entry
            .file_name()
            .to_str()
            .and_then(|name| Uuid::parse_str(name).ok())
            .ok_or(Error::Refused("Source context inventory is invalid."))?;
        let context: Context = read(&entry.path().join("context.json"))?;
        if context.workspace == workspace {
            let context = owned(workspace, context.generation, id)?;
            if context.phase != "deleted" {
                return Err(Error::Refused(
                    "Release this workspace's captured source contexts before deleting it.",
                ));
            }
            ensure_absent(&entry.path())?;
        }
    }
    Ok(())
}
