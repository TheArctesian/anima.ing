//! Who other than the person and root can change a path (MCP-033, MCP-034). A writable group
//! counts when it lists another member or its members cannot be read (R-442).

mod groups;
#[cfg(unix)]
mod system;

use std::fmt;
#[cfg(unix)]
use std::fs;
#[cfg(unix)]
use std::os::unix::fs::{MetadataExt as _, PermissionsExt as _};
use std::path::PathBuf;
#[cfg(unix)]
use std::path::{Component, Path};

pub use groups::{Groups, Listing, Member};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Writer {
    Owner(String),
    Everyone,
    Member { group: String, member: String },
    Unread { group: String },
    Unknown,
    AccessList,
}

impl fmt::Display for Writer {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Owner(owner) => write!(formatter, "belongs to the user {owner}"),
            Self::Everyone => formatter.write_str("can be changed by each user"),
            Self::Member { group, member } => write!(
                formatter,
                "can be changed by the group {group}, which also has the member {member}"
            ),
            Self::Unread { group } => write!(
                formatter,
                "can be changed by the group {group}, whose members could not be read"
            ),
            Self::Unknown => {
                formatter.write_str("cannot be checked, so who can change it is not known")
            }
            Self::AccessList => {
                formatter.write_str("has an access list that lets a different user change it")
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Others {
    pub path: PathBuf,
    pub writer: Writer,
}

impl fmt::Display for Others {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{} {}", self.path.display(), self.writer)
    }
}

/// The macOS group `admin`: its members can become root, so a root folder it can write counts as
/// root's.
#[cfg(unix)]
const MACOS_ADMIN: u32 = 80;

/// The most symbolic links that a path may pass through, as the kernel allows.
#[cfg(unix)]
const MOST_LINKS: usize = 40;

#[cfg(unix)]
#[derive(Clone, Copy, Debug)]
pub struct Part {
    pub owner: u32,
    pub group: u32,
    pub mode: u32,
    pub folder: bool,
    pub link: bool,
}

#[cfg(unix)]
impl Part {
    fn of(metadata: &fs::Metadata) -> Self {
        Self {
            owner: metadata.uid(),
            group: metadata.gid(),
            mode: metadata.permissions().mode(),
            folder: metadata.is_dir(),
            link: metadata.file_type().is_symlink(),
        }
    }
}

#[cfg(unix)]
#[must_use]
pub fn account(user: u32) -> String {
    system::account(user)
}

#[cfg(unix)]
#[must_use]
pub fn mode_writer(mode: u32, group: u32, user: u32, groups: &Groups) -> Option<Writer> {
    if mode & 0o002 != 0 {
        Some(Writer::Everyone)
    } else if mode & 0o020 != 0 {
        groups.writer(group, user)
    } else {
        None
    }
}

/// Who other than `user` and root can replace `part` of the path of a program (MCP-033).
#[cfg(unix)]
#[must_use]
pub fn program_writer(part: Part, user: u32, macos: bool, groups: &Groups) -> Option<Writer> {
    if part.owner != user && part.owner != 0 {
        return Some(Writer::Owner(account(part.owner)));
    }
    // The folder of a link decides who replaces it; a sticky folder lets only an entry's owner.
    if part.link || (part.folder && part.mode & 0o1000 != 0) {
        return None;
    }
    // Members of root's group, or of `admin` on macOS, can become root.
    let root_group = part.owner == 0 && (part.group == 0 || (macos && part.group == MACOS_ADMIN));
    if root_group {
        return (part.mode & 0o002 != 0).then_some(Writer::Everyone);
    }
    mode_writer(part.mode, part.group, user, groups)
}

/// A part that cannot be read counts as replaceable.
#[cfg(unix)]
fn above(path: &Path, user: u32, groups: &Groups) -> Option<Others> {
    path.ancestors().find_map(|part| {
        let writer = fs::symlink_metadata(part).map_or(Some(Writer::Unknown), |metadata| {
            program_writer(Part::of(&metadata), user, cfg!(target_os = "macos"), groups)
        });
        writer.map(|writer| Others {
            path: part.to_path_buf(),
            writer,
        })
    })
}

/// A new entry can shadow a program further on `PATH`; the sticky bit does not stop that, and a
/// folder that cannot be read counts as open.
#[cfg(unix)]
#[must_use]
pub fn creatable(folder: &Path, user: u32, groups: &Groups) -> Option<Others> {
    let writer = fs::metadata(folder).map_or(Some(Writer::Unknown), |metadata| {
        let part = Part::of(&metadata);
        let part = Part {
            mode: part.mode & !0o1000,
            ..part
        };
        program_writer(part, user, cfg!(target_os = "macos"), groups)
    });
    writer.map(|writer| Others {
        path: folder.to_path_buf(),
        writer,
    })
}

/// Each link met while the absolute `path` resolves, nested ones too, then its file; `None` when
/// it does not resolve or passes more than [`MOST_LINKS`] links.
#[cfg(unix)]
#[must_use]
pub fn chain(path: &Path) -> Option<Vec<PathBuf>> {
    fn parts(path: &Path) -> impl Iterator<Item = PathBuf> {
        let parts: Vec<PathBuf> = path
            .components()
            .map(|part| PathBuf::from(part.as_os_str()))
            .collect();
        parts.into_iter().rev()
    }
    if !path.is_absolute() {
        return None;
    }
    let mut left: Vec<PathBuf> = parts(path).collect();
    let mut at = PathBuf::from("/");
    let mut links = Vec::new();
    while let Some(part) = left.pop() {
        match part.components().next() {
            Some(Component::RootDir) => at = PathBuf::from("/"),
            Some(Component::ParentDir) => {
                at.pop();
            }
            Some(Component::Normal(name)) => {
                let next = at.join(name);
                if !fs::symlink_metadata(&next).ok()?.file_type().is_symlink() {
                    at = next;
                } else if links.len() < MOST_LINKS {
                    // A relative target resolves from the folder of the link, which `at` is.
                    left.extend(parts(&fs::read_link(&next).ok()?));
                    links.push(next);
                } else {
                    return None;
                }
            }
            _ => {}
        }
    }
    links.push(at);
    Some(links)
}

/// Counts each link of the [`chain`], the file it resolves to, and each folder above them.
#[cfg(unix)]
#[must_use]
pub fn replaceable(path: &Path, user: u32, groups: &Groups) -> Option<Others> {
    let Some(chain) = chain(path) else {
        return Some(Others {
            path: path.to_path_buf(),
            writer: Writer::Unknown,
        });
    };
    chain.iter().find_map(|part| above(part, user, groups))
}

#[cfg(all(test, unix))]
mod tests;
