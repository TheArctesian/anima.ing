//! A path is used only when it is a regular file inside the clone: a link to `~/.ssh` or elsewhere
//! is refused before it is uploaded (BLD-043, BLD-047).

use std::{
    ffi::OsString,
    fs::{self, File},
    io::{self, Read},
    path::{Path, PathBuf},
};

#[cfg(unix)]
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};

use super::{
    carried,
    error::BuilderError,
    job::{Target, Version},
};

/// The stable artefact prefix that the hub, the site and Section 3.10.4 use.
const PRODUCT: &str = "Anima";
/// `productName` in `tauri.conf.json`. Artefact names stay `Anima` (UPD-010), so
/// [`stabilize_names`] renames them.
const DISPLAY: &str = "Anima.ing";
const MAC_EXECUTABLE: &str = "anima-desktop";

/// Every step gets this folder as `CARGO_TARGET_DIR`, so a builder's own cargo `target-dir` cannot
/// move the bundle.
pub fn target_folder(clone: &Path) -> PathBuf {
    clone.join("target")
}

fn bundle_folder(clone: &Path, target: Target) -> PathBuf {
    match target {
        Target::LinuxX86_64 => target_folder(clone).join("release/bundle"),
        Target::WindowsX86_64 => target_folder(clone).join("x86_64-pc-windows-msvc/release/bundle"),
        Target::DarwinUniversal => {
            target_folder(clone).join("universal-apple-darwin/release/bundle")
        }
    }
}

pub fn updater_file(clone: &Path, target: Target, version: &Version) -> PathBuf {
    let folder = bundle_folder(clone, target);
    match target {
        Target::LinuxX86_64 => folder.join(format!(
            "appimage/{PRODUCT}_{}_amd64.AppImage",
            version.as_str()
        )),
        Target::DarwinUniversal => folder.join(format!("macos/{PRODUCT}.app.tar.gz")),
        Target::WindowsX86_64 => {
            folder.join(format!("nsis/{PRODUCT}_{}_x64-setup.exe", version.as_str()))
        }
    }
}

/// Made on macOS only (BLD-063).
pub fn dmg_file(clone: &Path, target: Target, version: &Version) -> Option<PathBuf> {
    match target {
        Target::LinuxX86_64 | Target::WindowsX86_64 => None,
        Target::DarwinUniversal => Some(
            bundle_folder(clone, target)
                .join(format!("dmg/{PRODUCT}_{}_universal.dmg", version.as_str())),
        ),
    }
}

pub fn mac_app(clone: &Path) -> PathBuf {
    bundle_folder(clone, Target::DarwinUniversal).join(format!("macos/{DISPLAY}.app"))
}

/// No file of an earlier attempt can be taken for this one's (D1 of the projects and builds
/// proposal).
/// A link among the folders above it is refused, never followed; a link at the folder is removed as
/// a link.
pub fn clear_bundle(clone: &Path, target: Target) -> Result<(), BuilderError> {
    let folder = bundle_folder(clone, target);
    let kind = match fs::symlink_metadata(&folder) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        other => other.map_err(|_| BuilderError::Bundle)?,
    };
    let parent = folder.parent().ok_or(BuilderError::Bundle)?;
    let root = fs::canonicalize(clone).map_err(|_| BuilderError::Bundle)?;
    let real = fs::canonicalize(parent).map_err(|_| BuilderError::Bundle)?;
    if real
        != root.join(
            parent
                .strip_prefix(clone)
                .map_err(|_| BuilderError::Bundle)?,
        )
    {
        return Err(BuilderError::Bundle);
    }
    if kind.is_dir() {
        fs::remove_dir_all(&folder)
    } else {
        fs::remove_file(&folder)
    }
    .map_err(|_| BuilderError::Bundle)
}

/// Gives the DMG and `AppImage` the stable `Anima_*` names (UPD-010). When Tauri wrote no
/// `productName`
/// file, the stable name must be there already (a commit whose `productName` was still `Anima`).
pub fn stabilize_names(
    clone: &Path,
    target: Target,
    version: &Version,
) -> Result<(), BuilderError> {
    let produced = match target {
        Target::LinuxX86_64 => bundle_folder(clone, target).join(format!(
            "appimage/{DISPLAY}_{}_amd64.AppImage",
            version.as_str()
        )),
        Target::WindowsX86_64 => bundle_folder(clone, target)
            .join(format!("nsis/{DISPLAY}_{}_x64-setup.exe", version.as_str())),
        Target::DarwinUniversal => bundle_folder(clone, target)
            .join(format!("dmg/{DISPLAY}_{}_universal.dmg", version.as_str())),
    };
    let wanted = match target {
        Target::LinuxX86_64 | Target::WindowsX86_64 => updater_file(clone, target, version),
        Target::DarwinUniversal => dmg_file(clone, target, version).ok_or(BuilderError::Bundle)?,
    };
    if wanted == produced || (!produced.exists() && wanted.is_file()) {
        return Ok(());
    }
    fs::rename(&produced, &wanted).map_err(|_| BuilderError::Bundle)
}

/// A regular file, not a link, inside `clone` once every link in its folders is followed.
pub fn own_file(clone: &Path, path: &Path) -> Result<PathBuf, BuilderError> {
    let kind = fs::symlink_metadata(path).map_err(|_| BuilderError::Bundle)?;
    let real = fs::canonicalize(path).map_err(|_| BuilderError::Bundle)?;
    let root = fs::canonicalize(clone).map_err(|_| BuilderError::Bundle)?;
    (kind.is_file() && real.starts_with(&root))
        .then_some(real)
        .ok_or(BuilderError::Bundle)
}

/// Never through a link, and only with one name, so a hard link to a file outside the clone is
/// refused (D-13).
/// Hash and read from this one descriptor, so the bytes recorded are the bytes checked.
pub fn open_own(clone: &Path, path: &Path) -> Result<File, BuilderError> {
    let real = own_file(clone, path)?;
    #[cfg(windows)]
    return anima_windows::files::read_single(&real).map_err(|_| BuilderError::Bundle);
    #[cfg(unix)]
    {
        let file = fs::OpenOptions::new()
            .read(true)
            .custom_flags(nix::libc::O_NOFOLLOW)
            .open(real)
            .map_err(|_| BuilderError::Bundle)?;
        let kind = file.metadata().map_err(|_| BuilderError::Bundle)?;
        (kind.is_file() && kind.nlink() == 1)
            .then_some(file)
            .ok_or(BuilderError::Bundle)
    }
}

/// `Contents/MacOS` must hold exactly it and the seven carried programs, so nothing unchecked is
/// notarized and shipped (INS-002, S12 2.3).
pub fn mac_executable(clone: &Path, app: &Path) -> Result<PathBuf, BuilderError> {
    let folder = own_folder(clone, &app.join("Contents/MacOS"))?;
    let entries = fs::read_dir(&folder).map_err(|_| BuilderError::Bundle)?;
    let mut names = entries
        .map(|entry| entry.map(|entry| entry.file_name()))
        .collect::<Result<Vec<OsString>, _>>()
        .map_err(|_| BuilderError::Bundle)?;
    names.sort();
    let mut expected: Vec<&str> = carried::programs().collect();
    expected.push(MAC_EXECUTABLE);
    expected.sort_unstable();
    if names != expected {
        return Err(BuilderError::Bundle);
    }
    own_file(clone, &folder.join(MAC_EXECUTABLE))
}

/// No link anywhere on the way from the clone, so a check of the files below it checks the build's
/// own files.
pub fn own_folder(clone: &Path, folder: &Path) -> Result<PathBuf, BuilderError> {
    let kind = fs::symlink_metadata(folder).map_err(|_| BuilderError::Bundle)?;
    let root = fs::canonicalize(clone).map_err(|_| BuilderError::Bundle)?;
    let real = fs::canonicalize(folder).map_err(|_| BuilderError::Bundle)?;
    let inside = folder
        .strip_prefix(clone)
        .map_err(|_| BuilderError::Bundle)?;
    (kind.is_dir() && real == root.join(inside))
        .then_some(real)
        .ok_or(BuilderError::Bundle)
}

/// Cargo gives its output a second name in `deps/`, so the one-name rule of [`open_own`] cannot
/// hold;
/// the staged file must then pass the `lipo` or ELF check, which no key or other text file passes.
pub fn open_built(clone: &Path, path: &Path) -> Result<File, BuilderError> {
    #[cfg(windows)]
    return anima_windows::files::read(&own_file(clone, path)?, false)
        .map_err(|_| BuilderError::Bundle);
    #[cfg(unix)]
    {
        let file = fs::OpenOptions::new()
            .read(true)
            .custom_flags(nix::libc::O_NOFOLLOW)
            .open(own_file(clone, path)?)
            .map_err(|_| BuilderError::Bundle)?;
        let kind = file.metadata().map_err(|_| BuilderError::Bundle)?;
        kind.is_file().then_some(file).ok_or(BuilderError::Bundle)
    }
}

/// What is at `to` is removed first and never followed, so a planted link cannot make the copy
/// write outside the clone.
pub fn stage(clone: &Path, from: &Path, to: &Path) -> Result<(), BuilderError> {
    let mut input = open_built(clone, from)?;
    let folder = to.parent().ok_or(BuilderError::Bundle)?;
    fs::create_dir_all(folder).map_err(|_| BuilderError::Bundle)?;
    let root = fs::canonicalize(clone).map_err(|_| BuilderError::Bundle)?;
    let real = fs::canonicalize(folder).map_err(|_| BuilderError::Bundle)?;
    if !real.starts_with(&root) {
        return Err(BuilderError::Bundle);
    }
    match fs::remove_file(to) {
        Err(error) if error.kind() != io::ErrorKind::NotFound => return Err(BuilderError::Bundle),
        _ => {}
    }
    #[cfg(windows)]
    let mut output = anima_windows::files::create_new(to).map_err(|_| BuilderError::Bundle)?;
    #[cfg(unix)]
    let mut output = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o755)
        .open(to)
        .map_err(|_| BuilderError::Bundle)?;
    io::copy(&mut input, &mut output).map_err(|_| BuilderError::Bundle)?;
    output.sync_all().map_err(|_| BuilderError::Bundle)
}

pub fn header(clone: &Path, path: &Path, count: u64) -> Result<Vec<u8>, BuilderError> {
    let mut bytes = Vec::new();
    open_own(clone, path)?
        .take(count)
        .read_to_end(&mut bytes)
        .map_err(|_| BuilderError::Bundle)?;
    Ok(bytes)
}

/// UPD-011. A third slice such as `x86_64h`, which an Intel Mac would prefer, is checked by nobody,
/// so it fails.
pub fn universal(archs: &[String]) -> bool {
    let mut words: Vec<&str> = archs
        .iter()
        .flat_map(|line| line.split_whitespace())
        .collect();
    words.sort_unstable();
    words == ["arm64", "x86_64"]
}

/// BLD-085. Progress lines say `Current status: ...`; only the final `status:` line counts.
pub fn notarized(lines: &[String]) -> bool {
    lines.iter().any(|line| line.trim() == "status: Accepted")
}
