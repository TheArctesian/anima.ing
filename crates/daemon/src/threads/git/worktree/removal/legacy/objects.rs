//! A private Git object view populated only with ordinary files named by the project's store.

use std::ffi::OsStr;
use std::io;
use std::path::Path;

use super::Dir;
use crate::threads::git::place::walk::Make;

pub(super) fn mirror(common: &Dir, view: &Path) -> io::Result<()> {
    let native = common
        .below(Path::new("objects"), Make::Existing)?
        .ok_or(io::ErrorKind::NotFound)?;
    let scratch = Dir::open(view)?;
    let objects = scratch
        .below(Path::new("objects"), Make::Existing)?
        .ok_or(io::ErrorKind::NotFound)?;
    for name in native.names()? {
        let Some(text) = name.to_str() else { continue };
        if text == "pack" || (text.len() == 2 && hex(text)) {
            mirror_directory(&native, &objects, &name)?;
        }
    }
    if let Some(shallow) = common.read(OsStr::new("shallow"), 16 * 1024 * 1024)? {
        std::fs::write(view.join("shallow"), shallow)?;
    }
    native.sync()
}

fn mirror_directory(native: &Dir, objects: &Dir, name: &OsStr) -> io::Result<()> {
    let relative = Path::new(name);
    let source = native
        .below(relative, Make::Existing)?
        .ok_or(io::ErrorKind::NotFound)?;
    let target = objects
        .below(relative, Make::Missing)?
        .ok_or(io::ErrorKind::NotFound)?;
    for file in source.names()? {
        if object_file(name, &file) {
            source.link_file(&file, &target)?;
        }
    }
    source.sync()
}

fn object_file(directory: &OsStr, name: &OsStr) -> bool {
    let Some(name) = name.to_str() else {
        return false;
    };
    if directory == "pack" {
        let Some((stem, kind)) = name.rsplit_once('.') else {
            return false;
        };
        return matches!(kind, "pack" | "idx")
            && stem
                .strip_prefix("pack-")
                .is_some_and(|oid| matches!(oid.len(), 40 | 64) && hex(oid));
    }
    matches!(name.len(), 38 | 62) && hex(name)
}

fn hex(name: &str) -> bool {
    name.bytes()
        .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}
