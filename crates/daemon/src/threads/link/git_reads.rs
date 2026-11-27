use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex, PoisonError};

use crate::threads::tree::identity::Process;
use crate::threads::wire::{Event, GitReadRoot};

#[derive(Clone)]
pub(super) struct GitReads {
    holder: Option<Process>,
    roots: Arc<Mutex<Roots>>,
}

enum Roots {
    Pending,
    Unavailable,
    Live {
        roots: BTreeMap<String, GitReadRoot>,
        last_generation: u64,
        suppressed: BTreeSet<u64>,
    },
}

impl GitReads {
    pub(super) fn new(holder: Option<Process>) -> Self {
        Self {
            holder,
            roots: Arc::new(Mutex::new(Roots::Pending)),
        }
    }

    pub(super) fn negotiated(&self) -> bool {
        self.holder.is_some()
    }

    pub(super) fn observe(&self, event: &Event) {
        let mut known = self.roots.lock().unwrap_or_else(PoisonError::into_inner);
        match event {
            Event::Hello {
                version,
                git_read_roots,
                ..
            } => {
                *known = if *version == crate::threads::wire::VERSION
                    && matches!(*known, Roots::Pending)
                    && self.negotiated()
                {
                    git_read_roots
                        .as_deref()
                        .and_then(Roots::from_hello)
                        .unwrap_or(Roots::Unavailable)
                } else {
                    Roots::Unavailable
                };
            }
            Event::GitReadRoot { root, enrolled } => known.update(root, *enrolled),
            _ => {}
        }
    }

    pub(super) fn suppress(&self, thread: &str) {
        let mut known = self.roots.lock().unwrap_or_else(PoisonError::into_inner);
        if let Roots::Live {
            roots, suppressed, ..
        } = &mut *known
            && let Some(root) = roots.get(thread)
        {
            suppressed.insert(root.generation);
        }
    }

    pub(super) fn disconnect(&self) {
        *self.roots.lock().unwrap_or_else(PoisonError::into_inner) = Roots::Unavailable;
    }
}

impl Roots {
    fn from_hello(roots: &[GitReadRoot]) -> Option<Self> {
        let mut known = BTreeMap::new();
        let mut generations = std::collections::BTreeSet::new();
        for root in roots {
            if root.generation == 0
                || !generations.insert(root.generation)
                || known.insert(root.thread.clone(), root.clone()).is_some()
            {
                return None;
            }
        }
        Some(Self::Live {
            roots: known,
            suppressed: BTreeSet::new(),
            last_generation: generations.last().copied().unwrap_or_default(),
        })
    }

    fn update(&mut self, root: &GitReadRoot, enrolled: bool) {
        let Self::Live {
            roots,
            last_generation,
            suppressed,
        } = self
        else {
            return;
        };
        if enrolled {
            if root.generation <= *last_generation || roots.contains_key(&root.thread) {
                *self = Self::Unavailable;
                return;
            }
            roots.insert(root.thread.clone(), root.clone());
            *last_generation = root.generation;
        } else if roots.get(&root.thread) == Some(root) {
            roots.remove(&root.thread);
            suppressed.remove(&root.generation);
        } else {
            *self = Self::Unavailable;
        }
    }
}

#[derive(Clone)]
pub(in crate::threads) struct LiveRoots {
    link: GitReads,
    pub holder: Process,
    pub roots: Vec<GitReadRoot>,
}

impl LiveRoots {
    pub(super) fn of(link: &GitReads) -> Option<Self> {
        let known = link.roots.lock().unwrap_or_else(PoisonError::into_inner);
        let Roots::Live {
            roots, suppressed, ..
        } = &*known
        else {
            return None;
        };
        Some(Self {
            link: link.clone(),
            holder: link.holder.clone()?,
            roots: roots
                .values()
                .filter(|root| !suppressed.contains(&root.generation))
                .cloned()
                .collect(),
        })
    }

    pub(super) fn with_current<T>(
        &self,
        link: &GitReads,
        root: &GitReadRoot,
        run: impl FnOnce() -> T,
    ) -> Option<T> {
        if !Arc::ptr_eq(&self.link.roots, &link.roots) {
            return None;
        }
        let known = link.roots.lock().unwrap_or_else(PoisonError::into_inner);
        let Roots::Live {
            roots, suppressed, ..
        } = &*known
        else {
            return None;
        };
        (roots.get(&root.thread) == Some(root) && !suppressed.contains(&root.generation)).then(run)
    }
}

#[cfg(test)]
mod tests;
