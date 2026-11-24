//! The 15-second work of the loads (K11 Section 11.1): the use of each copy from its counters, the copies that a
//! router lost, the unload of idle copies (FIT-022), and the return of one displaced pin (FIT-023). Also the orders
//! that end a mark: `model_unload`, `model_keep`, `copy_delete` and `server_stop`.

use std::sync::atomic::AtomicBool;

use daemon_core::model_work::cause::Cause;
use daemon_core::model_work::memory::OnServer;
use daemon_core::model_work::memory::keep::{Keep, Kept, MAX_FILES};
use daemon_core::model_work::memory::returns::{Displaced, Now, next};
use daemon_core::model_work::memory::room::{Load, idle};
use daemon_core::model_work::router::Status;
use uuid::Uuid;

use super::steps::{Ask, By};
use super::{At, Loads, kept};

impl Loads {
    /// Reads the counters and the list of each server: a request in progress makes a copy `active`, a request
    /// that ended since the last tick makes it used now, and a copy that its router no longer has loaded (a
    /// restart of the server) frees its reservation, and a pinned one waits for its return.
    async fn observe(&mut self, at: &At<'_>) {
        for (server, router) in at.routers {
            let Some(list) = router.statuses(false).await else {
                continue;
            };
            let gone: Vec<Uuid> = self
                .copies
                .values()
                .filter(|copy| {
                    copy.server == *server
                        && matches!(copy.held.state, OnServer::Ready | OnServer::Active)
                })
                .filter(|copy| !list.contains(&(copy.held.copy, Status::Loaded)))
                .map(|copy| copy.held.copy)
                .collect();
            for copy in gone {
                self.unload_one(at, copy, true).await;
            }
        }
        for copy in self.copies.values_mut() {
            if !matches!(copy.held.state, OnServer::Ready | OnServer::Active) {
                continue;
            }
            let Some(used) = (match at.routers.get(&copy.server) {
                Some(router) => router.usage(copy.held.copy).await,
                None => None,
            }) else {
                continue;
            };
            let busy = used.in_progress > 0;
            if busy || used.decodes != copy.decodes || copy.held.state == OnServer::Active {
                copy.held.used_at = at.now;
            }
            if used.decodes != copy.decodes {
                copy.held.returned_unused = false;
            }
            copy.decodes = used.decodes;
            copy.held.state = if busy {
                OnServer::Active
            } else {
                OnServer::Ready
            };
        }
    }

    /// One tick: the use, the idle unloads, then at most one return.
    pub async fn tick(&mut self, at: &At<'_>, permitted: bool, stop: &AtomicBool) {
        self.observe(at).await;
        for copy in idle(&self.held(), at.now) {
            self.unload_one(at, copy, false).await;
        }
        let waiting = self.waiting(at);
        let held = self.held();
        let node = Now {
            usable: at.usable,
            held: &held,
            busy_at: &self.busy_at,
            permitted,
        };
        let Some(copy) = next(&node, &waiting, at.now) else {
            return;
        };
        self.give_back(at, copy, stop).await;
    }

    /// The load of a return: the settings of its last load; a copy that a return loads is takeable at once.
    async fn give_back(&mut self, at: &At<'_>, copy: Uuid, stop: &AtomicBool) {
        let Some(kept) = self.kept.get(&copy).cloned() else {
            return;
        };
        let Some(settings) = kept.load_settings() else {
            return;
        };
        let Some(server) = kept.server else {
            return;
        };
        let ask = Ask {
            server,
            copy,
            settings: &settings,
            reserve_bytes: kept.reserve_bytes,
            keep: Some(kept.keep),
        };
        let Ok(mut on_node) = self.on_node(&ask, kept.keep, at.now) else {
            // A check of a load fails: the mark ends (Section 11.1).
            self.end_mark(copy);
            return;
        };
        on_node.held.returned_unused = true;
        if self.load_one(at, on_node, &By::Return(stop)).await.is_ok() {
            if let Some(kept) = self.kept.get_mut(&copy) {
                kept.displaced = false;
                kept.returned_at = Some(at.now);
            }
            let _ = self.save(copy);
        }
    }

    /// The displaced pins whose server is ready; a pin whose checks of a load fail ends its mark.
    fn waiting(&mut self, at: &At<'_>) -> Vec<Displaced> {
        let mut waiting = Vec::new();
        let mut failed = Vec::new();
        for (copy, kept) in &self.kept {
            if !kept.displaced || !kept.keep.pinned || self.copies.contains_key(copy) {
                continue;
            }
            let Some(server) = kept.server.filter(|server| at.routers.contains_key(server)) else {
                continue;
            };
            let Some(settings) = kept.load_settings() else {
                failed.push(*copy);
                continue;
            };
            let ask = Ask {
                server,
                copy: *copy,
                settings: &settings,
                reserve_bytes: kept.reserve_bytes,
                keep: Some(kept.keep),
            };
            match self.on_node(&ask, kept.keep, at.now) {
                Ok(on_node) => waiting.push(Displaced {
                    copy: *copy,
                    load: Load {
                        parts: on_node.held.parts,
                    },
                    returned_at: kept.returned_at,
                    interval: kept.interval,
                }),
                Err(_) => failed.push(*copy),
            }
        }
        for copy in failed {
            self.end_mark(copy);
        }
        waiting
    }

    fn end_mark(&mut self, copy: Uuid) {
        if let Some(kept) = self.kept.get_mut(&copy) {
            kept.displaced = false;
        }
        let _ = self.save(copy);
    }

    /// `model_unload`: unloads the copy if `server` has it, and ends its mark.
    ///
    /// # Errors
    /// `copy_unknown` when the node does not have the server.
    pub async fn unload(&mut self, at: &At<'_>, server: Uuid, copy: Uuid) -> Result<(), Cause> {
        if !at.routers.contains_key(&server) {
            return Err(Cause::CopyUnknown);
        }
        if self
            .copies
            .get(&copy)
            .is_some_and(|on_node| on_node.server == server)
        {
            self.unload_one(at, copy, false).await;
        }
        self.end_mark(copy);
        Ok(())
    }

    /// `model_keep`: the idle time and the pin of a copy on this node; a keep with no pin ends the mark.
    ///
    /// # Errors
    /// `copy_unknown` for a copy that is not on the node; `member_unknown` when the node holds 64 files.
    pub fn keep(&mut self, copy: Uuid, keep: Keep) -> Result<(), Cause> {
        let pulled = self
            .folders
            .models
            .join("copies")
            .join(copy.to_string())
            .is_dir();
        if !pulled && crate::model::found::record::read(&self.folders.state, copy).is_err() {
            return Err(Cause::CopyUnknown);
        }
        if !self.kept.contains_key(&copy) && self.kept.len() >= MAX_FILES {
            return Err(Cause::MemberUnknown);
        }
        let kept = self.kept.entry(copy).or_insert_with(|| Kept::new(keep));
        kept.keep = keep;
        if !keep.pinned {
            kept.displaced = false;
        }
        if let Some(on_node) = self.copies.get_mut(&copy) {
            on_node.held.keep = keep;
        }
        self.save(copy)
    }

    /// `copy_delete`: unloads the copy and deletes its keep file.
    ///
    /// # Errors
    /// `limit_disk` when the state folder cannot be written.
    pub async fn forget(&mut self, at: &At<'_>, copy: Uuid) -> Result<(), Cause> {
        self.unload_one(at, copy, false).await;
        self.kept.remove(&copy);
        kept::forget(&self.folders.state, copy)
    }

    /// `server_start`: the server starts again with no copy, so its copies free their reservations, and a pinned one
    /// waits for its return; a mark of an earlier run stays.
    pub fn server_started(&mut self, server: Uuid, now: u64) {
        let lost: Vec<(Uuid, bool, Vec<_>)> = self
            .copies
            .values()
            .filter(|copy| copy.server == server)
            .map(|copy| {
                (
                    copy.held.copy,
                    copy.held.keep.pinned,
                    copy.held.parts.clone(),
                )
            })
            .collect();
        for (copy, pinned, parts) in lost {
            self.copies.remove(&copy);
            if pinned {
                self.mark_displaced(copy, parts, now);
            }
        }
    }

    /// `server_stop`: the copies of the server free their reservations, and their marks end.
    pub fn server_stopped(&mut self, server: Uuid) {
        self.copies.retain(|_, copy| copy.server != server);
        let marked: Vec<Uuid> = self
            .kept
            .iter()
            .filter(|(_, kept)| kept.server == Some(server) && kept.displaced)
            .map(|(copy, _)| *copy)
            .collect();
        for copy in marked {
            self.end_mark(copy);
        }
    }
}
