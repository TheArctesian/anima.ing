//! The members of each group, read once (R-442).

use std::collections::HashMap;
use std::fmt;
use std::sync::{Arc, Mutex, PoisonError};

use super::Writer;

/// `uid` is `None` when no account has the name; such a member counts as a different user.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Member {
    pub name: String,
    pub uid: Option<u32>,
}

/// `members` is `None` when the system cannot say who is in the group.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Listing {
    pub name: String,
    pub members: Option<Vec<Member>>,
}

type Reader = dyn Fn(u32) -> Listing + Send + Sync;

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
    #[must_use]
    pub fn new(read: impl Fn(u32) -> Listing + Send + Sync + 'static) -> Self {
        Self {
            read: Arc::new(read),
            known: Arc::default(),
        }
    }

    /// Windows has no groups of this kind: each is unread.
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
