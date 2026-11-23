//! The members of each group, read once from a reader: the system's account database, or the
//! groups that a test gives (R-NEW-exactgroup).

use std::collections::HashMap;
use std::fmt;
use std::sync::{Arc, Mutex, PoisonError};

use super::Writer;

/// One member of a group: a name in the group's list of members, or an account whose own
/// (primary) group it is. `uid` is the number of the account with that name; `None` when no
/// account has the name, which then counts as a different user.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Member {
    pub name: String,
    pub uid: Option<u32>,
}

/// A group as the system lists it: its name (its number when it has none) and its members, or
/// `None` when the system cannot say who is in it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Listing {
    pub name: String,
    pub members: Option<Vec<Member>>,
}

type Reader = dyn Fn(u32) -> Listing + Send + Sync;

/// The groups of this computer, each read once by its number.
#[derive(Clone)]
pub struct Groups {
    read: Arc<Reader>,
    known: Arc<Mutex<HashMap<u32, Listing>>>,
}

impl fmt::Debug for Groups {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("Groups")
    }
}

impl Groups {
    /// The groups as `read` lists them.
    #[must_use]
    pub fn new(read: impl Fn(u32) -> Listing + Send + Sync + 'static) -> Self {
        Self {
            read: Arc::new(read),
            known: Arc::default(),
        }
    }

    /// The groups as this computer lists them. Windows has no groups of this kind: each is unread.
    #[must_use]
    pub fn system() -> Self {
        #[cfg(unix)]
        let read = super::system::listing;
        #[cfg(windows)]
        let read = |group: u32| Listing {
            name: group.to_string(),
            members: None,
        };
        Self::new(read)
    }

    /// Who other than `user` and root can write through the group `group`: its first member that
    /// is neither, or the group itself when its members cannot be read. `None`: only `user` and
    /// root are in it.
    #[must_use]
    pub fn writer(&self, group: u32, user: u32) -> Option<Writer> {
        let Listing { name, members } = self.listing(group);
        let Some(members) = members else {
            return Some(Writer::Unread { group: name });
        };
        members
            .into_iter()
            .find(|member| member.uid != Some(user) && member.uid != Some(0))
            .map(|member| Writer::Member {
                group: name,
                member: member.name,
            })
    }

    fn listing(&self, group: u32) -> Listing {
        let known = |groups: &Self| {
            groups
                .known
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .get(&group)
                .cloned()
        };
        if let Some(listing) = known(self) {
            return listing;
        }
        // Read without the lock: the reader may run a program of the system.
        let listing = (self.read)(group);
        self.known
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(group, listing.clone());
        listing
    }
}
