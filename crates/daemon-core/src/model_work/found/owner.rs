//! The owner rule of the folders of Section 10.2 (`contracts/compute/model-work.md` Section 2.3):
//! the daemon reads an entry only when no user but the user of the daemon, root and the owner of
//! the folder can change it. A group may write it only when that group is the private group of
//! the user of the daemon, as a home folder made with the umask 002 of Ubuntu and Pop!_OS has.

/// The owner, the group and the mode bits of an entry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Entry {
    pub owner: u32,
    pub group: u32,
    pub mode: u32,
}

/// The user of the daemon, and its private group when it has one: the group that only that user is in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Reader {
    pub user: u32,
    pub private_group: Option<u32>,
}

const GROUP_WRITE: u32 = 0o020;
const OTHER_WRITE: u32 = 0o002;

/// Whether the daemon may read `entry` in the folder of Section 10.2 that `folder_owner` owns:
/// owned by the user of the daemon, by root or by `folder_owner`; writable by no other user; and
/// writable by its group only when that group is the private group of the reader.
#[must_use]
pub const fn owner_permits(entry: Entry, reader: Reader, folder_owner: u32) -> bool {
    let owner_holds = entry.owner == reader.user || entry.owner == 0 || entry.owner == folder_owner;
    let group_private = match reader.private_group {
        Some(group) => group == entry.group,
        None => false,
    };
    let group_holds = entry.mode & GROUP_WRITE == 0 || group_private;
    owner_holds && entry.mode & OTHER_WRITE == 0 && group_holds
}
