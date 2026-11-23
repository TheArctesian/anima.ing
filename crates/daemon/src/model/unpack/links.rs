//! The symbolic links of an archive (Section 6, R-387): a link is kept until the last
//! entry is written, then replaced by a copy of the regular file of the archive that it names. A
//! target must be relative and stay in the archive; a chain of links ends at a file within
//! [`MAX_HOPS`] steps. The folder of the runtime never holds a link.

use std::collections::HashMap;
use std::fs::File;
use std::io::{self, Read};
use std::os::fd::OwnedFd;
use std::path::{Component, Path};

use daemon_core::model_work::cause::Cause;
use rustix::fs::{Mode, OFlags};

use super::{BAD_ARCHIVE, DIR_FLAGS, Kind, Writer, open_dir};

/// The most links that one link may pass through to reach its file.
const MAX_HOPS: usize = 8;

/// The longest target of a link that the unpack reads (the `PATH_MAX` of Linux).
const MAX_TARGET_BYTES: u64 = 4096;

impl<F: Fn() -> bool> Writer<F> {
    /// Keeps the link at `path` to `target` for [`Writer::finish_links`].
    pub(super) fn link(&mut self, path: &str, target: &str) -> Result<(), Cause> {
        let names = self.claim(path, false)?;
        self.links.push((names, target.to_owned()));
        Ok(())
    }

    /// Writes a copy of its file in the place of each link, in the order of the archive.
    pub(super) fn finish_links(&mut self) -> Result<(), Cause> {
        let links = std::mem::take(&mut self.links);
        let targets: HashMap<String, &str> = links
            .iter()
            .map(|(names, target)| (names.join("/"), target.as_str()))
            .collect();
        for (names, _) in &links {
            let source = resolve(names, &targets, &self.files).ok_or(BAD_ARCHIVE)?;
            let mode = self.files[&source];
            let mut file = open_file(&self.root, &source).map_err(|_| BAD_ARCHIVE)?;
            self.place(names, Kind::File(mode), &mut file)?;
        }
        Ok(())
    }
}

/// Reads the target of a `.zip` link: its bytes, UTF-8, at most [`MAX_TARGET_BYTES`].
pub(super) fn read_target(bytes: &mut dyn Read) -> Result<String, Cause> {
    let mut target = Vec::new();
    bytes
        .take(MAX_TARGET_BYTES + 1)
        .read_to_end(&mut target)
        .map_err(|_| BAD_ARCHIVE)?;
    if target.len() as u64 > MAX_TARGET_BYTES {
        return Err(BAD_ARCHIVE);
    }
    String::from_utf8(target).map_err(|_| BAD_ARCHIVE)
}

/// The path of the regular file that the link at `link` names, through at most [`MAX_HOPS`]
/// links of `targets`; `None` for a target that leaves the archive or names no file.
fn resolve(
    link: &[String],
    targets: &HashMap<String, &str>,
    files: &HashMap<String, u32>,
) -> Option<String> {
    let mut at = link.to_vec();
    for _ in 0..MAX_HOPS {
        at = join(&at, targets.get(&at.join("/"))?)?;
        let path = at.join("/");
        if files.contains_key(&path) {
            return Some(path);
        }
    }
    None
}

/// The names of `target` read from the folder of the link at `link`; `None` for a target that
/// is empty, absolute, ends in `/`, holds a `\` or leaves the archive by `..`.
fn join(link: &[String], target: &str) -> Option<Vec<String>> {
    if target.is_empty() || target.ends_with('/') || target.contains('\\') {
        return None;
    }
    let mut names = link.get(..link.len().checked_sub(1)?)?.to_vec();
    for part in Path::new(target).components() {
        match part {
            Component::Normal(name) => names.push(name.to_str()?.to_owned()),
            Component::CurDir => {}
            Component::ParentDir => drop(names.pop()?),
            Component::RootDir | Component::Prefix(_) => return None,
        }
    }
    (!names.is_empty()).then_some(names)
}

/// Opens the file at `path` below `root` for reading, with no link followed at any part.
fn open_file(root: &OwnedFd, path: &str) -> io::Result<File> {
    let names: Vec<&str> = path.split('/').collect();
    let (last, parents) = names.split_last().ok_or(io::ErrorKind::InvalidInput)?;
    let mut dir = rustix::fs::openat(root, ".", DIR_FLAGS, Mode::empty())?;
    for name in parents {
        dir = open_dir(&dir, name, false)?;
    }
    let flags = OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC;
    Ok(File::from(rustix::fs::openat(
        &dir,
        *last,
        flags,
        Mode::empty(),
    )?))
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::resolve;

    fn names(path: &str) -> Vec<String> {
        path.split('/').map(str::to_owned).collect()
    }

    fn resolved(link: &str, targets: &[(&str, &str)]) -> Option<String> {
        let targets: HashMap<String, &str> = targets
            .iter()
            .map(|(at, to)| ((*at).to_owned(), *to))
            .collect();
        let files = HashMap::from([
            ("lib/libllama.so.1".to_owned(), 0o755),
            ("README".to_owned(), 0o644),
        ]);
        resolve(&names(link), &targets, &files)
    }

    #[test]
    fn a_relative_link_reaches_its_file_directly_or_through_links() {
        let chain = [
            ("lib/libllama.so", "libllama.so.1"),
            ("bin/libllama.so", "../lib/./libllama.so"),
        ];
        assert_eq!(
            resolved("lib/libllama.so", &chain).as_deref(),
            Some("lib/libllama.so.1")
        );
        assert_eq!(
            resolved("bin/libllama.so", &chain).as_deref(),
            Some("lib/libllama.so.1")
        );
        assert_eq!(
            resolved("readme", &[("readme", "README")]).as_deref(),
            Some("README")
        );
    }

    #[test]
    fn a_link_that_leaves_the_archive_or_names_no_file_is_refused() {
        let refused: [&[(&str, &str)]; 7] = [
            &[("lib/a", "/etc/passwd")],
            &[("lib/a", "../../etc/passwd")],
            &[("lib/a", "")],
            &[("lib/a", "missing")],
            &[("lib/a", "..")],
            &[("lib/a", "libllama.so.1/")],
            &[("lib/a", "b"), ("lib/b", "a")],
        ];
        for (index, targets) in refused.iter().enumerate() {
            assert_eq!(resolved("lib/a", targets), None, "case {index}");
        }
    }
}
