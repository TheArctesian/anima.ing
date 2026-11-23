use std::fs::File;

use daemon_core::model_work::cause::Cause;
use flate2::read::GzDecoder;
use tar::EntryType;

use super::{BAD_ARCHIVE, Kind, Writer};

pub(super) fn unpack<F: Fn() -> bool>(archive: File, writer: &mut Writer<F>) -> Result<(), Cause> {
    let mut tar = tar::Archive::new(GzDecoder::new(archive));
    for entry in tar.entries().map_err(|_| BAD_ARCHIVE)? {
        writer.next_entry()?;
        let mut entry = entry.map_err(|_| BAD_ARCHIVE)?;
        let path = entry.path().map_err(|_| BAD_ARCHIVE)?;
        let path = path.to_str().ok_or(BAD_ARCHIVE)?.to_owned();
        let kind = match entry.header().entry_type() {
            EntryType::Symlink => {
                let target = entry.link_name().map_err(|_| BAD_ARCHIVE)?;
                let target = target.as_deref().and_then(|target| target.to_str());
                writer.link(&path, target.ok_or(BAD_ARCHIVE)?)?;
                continue;
            }
            EntryType::Directory => Kind::Folder,
            EntryType::Regular | EntryType::Continuous => {
                Kind::File(entry.header().mode().map_err(|_| BAD_ARCHIVE)?)
            }
            _ => return Err(BAD_ARCHIVE),
        };
        writer.write(&path, kind, &mut entry)?;
    }
    Ok(())
}
