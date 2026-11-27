//! Reconcile private Compose files under the application-store lock.
//! Only the normal apply path runs Compose. Recovery follows the saved result of that command.

use std::io;

use super::{Fault, files::Store};
use crate::{Folders, plan::Version};

const JOURNAL: &str = ".version-transition";
const PREVIOUS_FORM: &str = ".previous-final-form.json";

enum Transition {
    Starting { had_form: bool },
    Confirmed { keep_previous: bool },
}

impl Transition {
    fn encoding(&self) -> &'static [u8] {
        match self {
            Self::Starting { had_form: false } => b"starting:new\n",
            Self::Starting { had_form: true } => b"starting:update\n",
            Self::Confirmed {
                keep_previous: false,
            } => b"confirmed:discard\n",
            Self::Confirmed {
                keep_previous: true,
            } => b"confirmed:keep\n",
        }
    }

    fn read(store: &Store<'_>, folders: &Folders) -> Result<Option<Self>, Fault> {
        let path = folders.file(JOURNAL);
        if !store.has_private_file(&path)? {
            return Ok(None);
        }
        let bytes = store.read_small_file(&path, 32)?;
        let transition = match bytes.as_slice() {
            b"starting:new\n" => Self::Starting { had_form: false },
            b"starting:update\n" => Self::Starting { had_form: true },
            b"confirmed:discard\n" => Self::Confirmed {
                keep_previous: false,
            },
            b"confirmed:keep\n" => Self::Confirmed {
                keep_previous: true,
            },
            _ => return Err(incomplete()),
        };
        Ok(Some(transition))
    }

    fn write(&self, store: &Store<'_>, folders: &Folders) -> Result<(), Fault> {
        store.write(&folders.file(JOURNAL), self.encoding())
    }
}

pub(super) fn apply(
    store: &Store<'_>,
    version: &Version,
    start: impl FnOnce() -> Result<(), Fault>,
) -> Result<(), Fault> {
    let folders = &version.folders;
    recover(store, folders)?;
    let had_form = store.has_private_file(&folders.final_form())?;
    if version.keep_previous {
        for file in folders.current() {
            require_file(store, &file)?;
        }
    }

    Transition::Starting { had_form }.write(store, folders)?;
    if let Err(fault) = stage(store, version, had_form).and_then(|()| start()) {
        recover(store, folders)?;
        return Err(fault);
    }
    let confirmed = Transition::Confirmed {
        keep_previous: version.keep_previous,
    };
    confirmed
        .write(store, folders)
        .and_then(|()| recover(store, folders))
        .map_err(|fault| {
            eprintln!("application version recording: {fault}");
            Fault::VersionUnconfirmed
        })
}

fn stage(store: &Store<'_>, version: &Version, had_form: bool) -> Result<(), Fault> {
    let folders = &version.folders;
    let [compose, environment, digests] = folders.staged();
    store.write(&compose, &version.compose)?;
    store.write(&environment, &version.environment)?;
    store.write(&digests, &version.digests)?;
    let final_form = folders.final_form();
    if had_form {
        store.copy_file(&final_form, &folders.file(PREVIOUS_FORM))?;
    }
    store.write(&final_form, &version.final_form)
}

pub(crate) fn recover(store: &Store<'_>, folders: &Folders) -> Result<(), Fault> {
    match Transition::read(store, folders)? {
        None => return Ok(()),
        Some(Transition::Starting { had_form }) => restore(store, folders, had_form)?,
        Some(Transition::Confirmed { keep_previous }) => finish(store, folders, keep_previous)?,
    }
    store.delete_file(&folders.file(JOURNAL))
}

pub(crate) fn require_settled(store: &Store<'_>, folders: &Folders) -> Result<(), Fault> {
    if Transition::read(store, folders)?.is_some() {
        return Err(Fault::VersionUnconfirmed);
    }
    Ok(())
}

fn restore(store: &Store<'_>, folders: &Folders, had_form: bool) -> Result<(), Fault> {
    let previous = folders.file(PREVIOUS_FORM);
    let final_form = folders.final_form();
    if store.has_private_file(&previous)? {
        store.move_file(&previous, &final_form)?;
    } else if had_form {
        require_file(store, &final_form)?;
    } else {
        store.delete_file(&final_form)?;
    }
    for file in folders.staged() {
        store.delete_file(&file)?;
    }
    Ok(())
}

fn finish(store: &Store<'_>, folders: &Folders, keep_previous: bool) -> Result<(), Fault> {
    require_file(store, &folders.final_form())?;
    for ((staged, current), kept) in folders
        .staged()
        .iter()
        .zip(folders.current())
        .zip(folders.kept())
    {
        if store.has_private_file(staged)? {
            if keep_previous {
                // A missing current file means it was already moved to the kept path.
                store.move_file(&current, &kept)?;
                require_file(store, &kept)?;
            } else {
                store.delete_file(&kept)?;
            }
            store.move_file(staged, &current)?;
        }
        // A missing staged file means this record was already promoted.
        require_file(store, &current)?;
        if keep_previous {
            require_file(store, &kept)?;
        }
    }
    store.delete_file(&folders.file(PREVIOUS_FORM))
}

fn require_file(store: &Store<'_>, path: &str) -> Result<(), Fault> {
    if store.has_private_file(path)? {
        Ok(())
    } else {
        Err(incomplete())
    }
}

fn incomplete() -> Fault {
    Fault::Storage(io::ErrorKind::InvalidData)
}
