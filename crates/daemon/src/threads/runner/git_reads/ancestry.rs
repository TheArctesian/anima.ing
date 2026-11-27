use uuid::Uuid;

use crate::threads::link::LiveRoots;
use crate::threads::tree::identity::{Identity, Process};
use crate::threads::wire::GitReadRoot;

const ANCESTORS: usize = 64;

pub(super) struct Binding {
    pub(super) roots: LiveRoots,
    pub(super) root: GitReadRoot,
    pub(super) thread: Uuid,
    pins: Vec<Process>,
}

impl Binding {
    pub(super) fn find(roots: LiveRoots, peer: Process) -> Option<Self> {
        let holder = roots.holder.identity();
        let mut at = peer;
        let mut pins = Vec::new();
        let mut found = None;
        for _ in 0..ANCESTORS {
            let identity = at.identity();
            if identity == holder
                || pins
                    .iter()
                    .any(|pin: &Process| pin.identity().pid == identity.pid)
            {
                return None;
            }
            let parent = at.parent()?;
            if let Some(root) = enrolled(&roots.roots, identity).ok()? {
                if found.is_some() || parent.identity() != holder {
                    return None;
                }
                found = Some(root.clone());
            }
            pins.push(at);
            if parent.identity() == holder {
                return Self::proven(roots, found?, pins, parent);
            }
            at = parent;
        }
        None
    }

    fn proven(
        roots: LiveRoots,
        root: GitReadRoot,
        mut pins: Vec<Process>,
        parent: Process,
    ) -> Option<Self> {
        pins.push(parent);
        let thread = Uuid::parse_str(&root.thread).ok()?;
        let binding = Self {
            roots,
            root,
            thread,
            pins,
        };
        binding.live().then_some(binding)
    }

    pub(super) fn live(&self) -> bool {
        self.roots.holder.live() && self.pins.iter().all(Process::live)
    }
}

/// An unrelated process may have the same PID as a dead root. It is not the root.
fn enrolled(roots: &[GitReadRoot], identity: Identity) -> Result<Option<&GitReadRoot>, ()> {
    let mut matching = roots.iter().filter(|root| root.process.pid == identity.pid);
    let first = matching.next();
    if matching.next().is_some() || first.is_some_and(|root| root.process != identity) {
        return Err(());
    }
    Ok(first)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tc7_w3_a_replacement_or_ambiguous_root_does_not_enroll_at_the_same_pid() {
        let original = Identity {
            pid: 41,
            device: 16,
            inode: 100,
        };
        let root = GitReadRoot {
            thread: Uuid::from_u128(1).to_string(),
            process: original,
            generation: 1,
        };
        let registered = std::slice::from_ref(&root);
        assert_eq!(enrolled(registered, original), Ok(Some(&root)));
        assert!(
            enrolled(
                registered,
                Identity {
                    inode: 101,
                    ..original
                }
            )
            .is_err()
        );
        assert!(enrolled(&[root.clone(), root], original).is_err());
    }
}
