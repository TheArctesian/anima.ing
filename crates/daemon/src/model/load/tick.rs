//! The 15-second work of the loads (K11 Section 11.1): the use of each copy from its counters, the copies that a
//! router lost, the unload of idle copies (FIT-022), and the return of one displaced pin (FIT-023).

use std::sync::atomic::{AtomicBool, Ordering};

use daemon_core::model_work::cause::Cause;
use daemon_core::model_work::memory::OnServer;
use daemon_core::model_work::memory::returns::{Displaced, Now, next};
use daemon_core::model_work::memory::room::{Load, idle};
use daemon_core::model_work::router::Status;
use uuid::Uuid;

use super::steps::{Ask, By};
use super::{At, Loads};

impl Loads {
    /// The copies that their servers no longer hold: a server that is not ready lost its copies (a restart, a
    /// stop, a failure), and a router can unload a copy itself. A copy that the router marks failed crashed
    /// (`load_crash`), so its mark ends; each other pinned copy waits for its return.
    async fn lost(&mut self, at: &At<'_>) {
        let mut lost: Vec<(Uuid, bool)> = Vec::new();
        for copy in self.copies.values() {
            if !matches!(copy.held.state, OnServer::Ready | OnServer::Active) {
                continue;
            }
            let Some(router) = at.routers.get(&copy.server) else {
                lost.push((copy.held.copy, true));
                continue;
            };
            let Some(listed) = router.statuses(false).await else {
                continue;
            };
            match listed.iter().find(|(id, _)| *id == copy.held.copy) {
                Some((_, Status::Loaded)) => {}
                Some((_, Status::Failed)) => lost.push((copy.held.copy, false)),
                _ => lost.push((copy.held.copy, true)),
            }
        }
        for (copy, displaced) in lost {
            self.drop_copy(copy, displaced, at.now);
            if !displaced {
                self.end_mark(copy);
            }
        }
    }

    /// Reads the counters of each copy: a request in progress makes a copy `active`, and a request that ended
    /// since the last read makes it used now. The steps of a `model_load` call it too, so that a copy with a
    /// request in progress is not taken for room.
    pub(super) async fn observe(&mut self, at: &At<'_>) {
        self.lost(at).await;
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

    /// One tick: the use, the idle unloads, then at most one return. An unload that the router does not confirm
    /// is tried again at the next tick.
    pub async fn tick(&mut self, at: &At<'_>, permitted: bool, stop: &AtomicBool) {
        self.observe(at).await;
        for copy in idle(&self.held(), at.now) {
            if stop.load(Ordering::SeqCst) {
                return;
            }
            let _ = self.unload_one(at, copy, false).await;
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

    /// The load of a return: the settings of its last load; a copy that a return loads is takeable at once. A
    /// return that crashes ends the mark (Section 6: the daemon does not load a `load_crash` copy by itself); one
    /// that fails otherwise waits a longer interval; one that gave way to an order waits for the next tick.
    async fn give_back(&mut self, at: &At<'_>, copy: Uuid, stop: &AtomicBool) {
        let Some(kept) = self.kept.get(&copy).cloned() else {
            return;
        };
        let (Some(settings), Some(server)) = (kept.load_settings(), kept.server) else {
            self.end_mark(copy);
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
        let parts = on_node.held.parts.clone();
        let done = self.load_one(at, on_node, &By::Return(stop)).await;
        match done {
            Ok(()) => {
                if let Some(kept) = self.kept.get_mut(&copy) {
                    kept.displaced = false;
                    kept.returned_at = Some(at.now);
                }
                let _ = self.save(copy);
            }
            Err(Cause::LoadCrash) => self.end_mark(copy),
            Err(_) if stop.load(Ordering::SeqCst) => {}
            Err(_) => {
                if let Some(kept) = self.kept.get_mut(&copy) {
                    let mark =
                        Displaced::new(copy, Load { parts }, Some((at.now, kept.interval)), at.now);
                    kept.returned_at = Some(at.now);
                    kept.interval = mark.interval;
                }
                let _ = self.save(copy);
            }
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
}
