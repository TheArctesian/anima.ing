use super::{Context, Error, Identity, ROOT, Result, read, record};
use std::{
    fs,
    io::Write,
    os::unix::fs::{DirBuilderExt, OpenOptionsExt},
    path::{Path, PathBuf},
};
use uuid::Uuid;

pub struct Pinned {
    pub directory: PathBuf,
    pub digest: String,
    pub checkout: Option<super::super::checkout::Source>,
}

pub fn pin(_allocation: &record::Allocation, source: Identity, attempt: Uuid) -> Result<Pinned> {
    let Identity {
        workspace,
        generation,
        context,
    } = source;
    let base = Path::new(ROOT).join(context.to_string());
    let receipt: Context = read(&base.join("context.json"))?;
    if receipt.schema != 1
        || receipt.id != context
        || receipt.workspace != workspace
        || receipt.generation != generation
        || receipt.phase != "captured"
    {
        return Err(Error::Refused(
            "The source context is not captured for this workspace generation.",
        ));
    }
    if let Some(checkout) = &receipt.source {
        checkout.authorize()?;
    }
    let digest = receipt
        .digest
        .ok_or(Error::Refused("The source digest is missing."))?;
    let source = base.join("source");
    if !fs::symlink_metadata(&source)?.is_dir() {
        return Err(Error::Refused("The captured source is unavailable."));
    }
    let leases = base.join("leases");
    if !leases.try_exists()? {
        fs::DirBuilder::new().mode(0o700).create(&leases)?;
    }
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .custom_flags(nix::libc::O_NOFOLLOW)
        .open(leases.join(attempt.to_string()))?;
    file.write_all(attempt.to_string().as_bytes())?;
    file.sync_all()?;
    fs::File::open(leases)?.sync_all()?;
    Ok(Pinned {
        directory: source,
        digest,
        checkout: receipt.source,
    })
}

pub fn unpin(_allocation: &record::Allocation, context: Uuid, attempt: Uuid) -> Result<()> {
    let leases = Path::new(ROOT).join(context.to_string()).join("leases");
    match fs::remove_file(leases.join(attempt.to_string())) {
        Ok(()) => fs::File::open(leases)?.sync_all()?,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => (),
        Err(error) => return Err(error.into()),
    }
    Ok(())
}
