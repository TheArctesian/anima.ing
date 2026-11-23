//! Who other than the person and root can change a path: a different owner, each user, or a
//! member of a group that can write it (MCP-033, MCP-034). A group counts only when the system
//! lists a member of it other than the person and root; when the system cannot say who is in it,
//! it counts (R-442).

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

/// Who other than the person and root can change one part of a path.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Writer {
    /// A different user owns it: the name of that user, or its number.
    Owner(String),
    /// Each user can write it.
    Everyone,
    /// A group that can write it has this member, who is not the person or root.
    Member { group: String, member: String },
    /// A group can write it, and the system did not say who is in that group.
    Unread { group: String },
    /// It cannot be read, does not resolve or starts a program in a way that is not read, so who
    /// can change it is not known.
    Unknown,
    /// Windows: its access list lets a different user change it.
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

/// A part of a path and who other than the person and root can change it.
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

/// The group `admin` of macOS. Its members can become root, so a folder of root that this group
/// can write (as `/Applications`) is as fixed as a folder of root alone.
#[cfg(unix)]
const MACOS_ADMIN: u32 = 80;

/// The most symbolic links that a path may pass through, as the kernel allows.
#[cfg(unix)]
const MOST_LINKS: usize = 40;

/// One part of a path as [`program_writer`] reads it.
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

/// The name of the user with the number `user`, or the number when it has no name.
#[cfg(unix)]
#[must_use]
pub fn account(user: u32) -> String {
    system::account(user)
}

/// Who other than `user` and root can change a part through its mode: each user when all may
/// write it, or a member of its group when the group may write it.
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
/// `macos`: a folder of root that the group `admin` can write counts as root's.
#[cfg(unix)]
#[must_use]
pub fn program_writer(part: Part, user: u32, macos: bool, groups: &Groups) -> Option<Writer> {
    if part.owner != user && part.owner != 0 {
        return Some(Writer::Owner(account(part.owner)));
    }
    // A link has no mode of its own; the folder that holds it decides who replaces it. A folder
    // with the sticky bit (as /tmp) lets only the owner of an entry replace it.
    if part.link || (part.folder && part.mode & 0o1000 != 0) {
        return None;
    }
    // A part of root that root's own group, or on macOS the group admin, can write stays as
    // fixed as before the exact check: those members can become root.
    let root_group = part.owner == 0 && (part.group == 0 || (macos && part.group == MACOS_ADMIN));
    if root_group {
        return (part.mode & 0o002 != 0).then_some(Writer::Everyone);
    }
    mode_writer(part.mode, part.group, user, groups)
}

/// The first part of `path`, from the thing itself up to the root, that a different user than
/// `user` can replace. A part that cannot be read counts as replaceable.
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

/// Who other than `user` and root can make a new entry in `folder`, as a program of the same name
/// as one further on `PATH`. The sticky bit does not stop that: it only keeps an entry that is
/// there. A folder that cannot be read counts as open.
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

/// Each path that decides what runs at the absolute `path`, in order: each symbolic link met while
/// the path resolves, also a link inside a link, and the file it resolves to at the end. `None`
/// when it does not resolve or passes more than [`MOST_LINKS`] links.
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

/// Who other than `user` and root can change what runs at the absolute `path`: each link of its
/// [`chain`], the file it resolves to, or a folder above any of them.
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
