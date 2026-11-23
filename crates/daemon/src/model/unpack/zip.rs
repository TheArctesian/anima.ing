//! The entries of a `.zip` (R-329), with the rules of the `.tar.gz`: only a regular
//! file, a folder or a symbolic link (the Unix type bits, when the entry has them, say which;
//! the bytes of a link are its target, kept for [`super::links`]), no encrypted entry, the declared size and the bytes read both within the limit, so a zip
//! bomb stops at the limit.

use std::fs::File;

use daemon_core::model_work::cause::Cause;

use super::{BAD_ARCHIVE, Kind, Writer};

/// The type bits of a Unix mode, and the two types that an entry may have.
const TYPE_BITS: u32 = 0o170_000;
const REGULAR: u32 = 0o100_000;
const FOLDER: u32 = 0o040_000;
const LINK: u32 = 0o120_000;

pub(super) fn unpack<F: Fn() -> bool>(archive: File, writer: &mut Writer<F>) -> Result<(), Cause> {
    let mut zip = ::zip::ZipArchive::new(archive).map_err(|_| BAD_ARCHIVE)?;
    if zip.len() > writer.max_entries {
        return Err(Cause::LimitDisk);
    }
    for index in 0..zip.len() {
        writer.next_entry()?;
        let mut entry = zip.by_index(index).map_err(|_| BAD_ARCHIVE)?;
        let mode = entry.unix_mode().unwrap_or(0);
        let type_bits = mode & TYPE_BITS;
        if entry.encrypted() || ![0, REGULAR, FOLDER, LINK].contains(&type_bits) {
            return Err(BAD_ARCHIVE);
        }
        if entry.size() > writer.left {
            return Err(Cause::LimitDisk);
        }
        let path = entry.name().map_err(|_| BAD_ARCHIVE)?.into_owned();
        if entry.is_symlink() || type_bits == LINK {
            let target = super::links::read_target(&mut entry)?;
            writer.link(&path, &target)?;
            continue;
        }
        let folder = entry.is_dir() || type_bits == FOLDER;
        let kind = if folder {
            Kind::Folder
        } else {
            Kind::File(mode)
        };
        writer.write(&path, kind, &mut entry)?;
    }
    Ok(())
}
