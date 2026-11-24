//! `model_load` and the unload of one copy (K11 Section 11.1, steps 1 to 4).

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};

use daemon_core::model_work::cause::Cause;
use daemon_core::model_work::memory::keep::{Keep, Kept, MAX_FILES, reservation};
use daemon_core::model_work::memory::returns::Displaced;
use daemon_core::model_work::memory::room::{Load, Room, room};
use daemon_core::model_work::memory::{GpuShare, Held, Memory, OnServer, llama_parts};
use daemon_core::model_work::router::{Entry, Status, model_files, preset};
use daemon_core::model_work::settings::{self, ServerSettings};
use uuid::Uuid;

use super::router::Waited;
use super::{At, Loads, OnNode, kept};
use crate::model::cycle::PRESET;
use crate::model::folders::write_private;

/// What a `model_load` asks.
pub struct Ask<'a> {
    pub server: Uuid,
    pub copy: Uuid,
    pub settings: &'a ServerSettings,
    pub reserve_bytes: Option<u64>,
    pub keep: Option<Keep>,
}

/// How a load starts: by a `model_load`, or by a return that gives way when `stop` is set.
pub enum By<'a> {
    Order,
    Return(&'a AtomicBool),
}

impl Loads {
    /// The model file and the projector of a pulled copy.
    fn files_of(&self, copy: Uuid) -> Result<(PathBuf, Option<PathBuf>), Cause> {
        let folder = self.folders.models.join("copies").join(copy.to_string());
        let Ok(entries) = std::fs::read_dir(&folder) else {
            // TODO(L1-16 follow-up, MW-P15): a found copy reaches a router only by a path, which Section 2.3
            // does not permit, so this version does not load it.
            let found = crate::model::found::record::read(&self.folders.state, copy).is_ok();
            return Err(if found {
                Cause::CopyNotReady
            } else {
                Cause::CopyUnknown
            });
        };
        let names: Vec<String> = entries
            .filter_map(Result::ok)
            .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_file()))
            .filter_map(|entry| entry.file_name().into_string().ok())
            .collect();
        let (model, projector) = model_files(&names).ok_or(Cause::CopyNotReady)?;
        Ok((folder.join(model), projector.map(|name| folder.join(name))))
    }

    /// Writes the preset of `server` with each copy that it holds.
    fn write_preset(&self, server: Uuid) -> Result<(), Cause> {
        let entries: Vec<Entry<'_>> = self
            .copies
            .values()
            .filter(|copy| copy.server == server)
            .map(|copy| Entry {
                copy: copy.held.copy,
                model: &copy.model,
                projector: copy.projector.as_deref(),
                settings: &copy.settings,
            })
            .collect();
        let text = preset(&entries).ok_or(Cause::CopyNotReady)?;
        write_private(&self.folders.server(server), "", PRESET, text.as_bytes())
            .map_err(|_| Cause::LimitDisk)
    }

    pub(super) fn save(&self, copy: Uuid) -> Result<(), Cause> {
        self.kept
            .get(&copy)
            .map_or(Ok(()), |kept| kept::write(&self.folders.state, copy, kept))
    }

    /// Unloads `copy` and frees its reservation; a pinned copy that `displaced` names waits for its return. `false`
    /// when the router did not confirm the unload: the copy keeps its reservation, and a later step tries again.
    /// A server that is not ready no longer holds the copy.
    pub(super) async fn unload_one(&mut self, at: &At<'_>, copy: Uuid, displaced: bool) -> bool {
        let Some(on_node) = self.copies.get_mut(&copy) else {
            return true;
        };
        let (server, before) = (on_node.server, on_node.held.state);
        let Some(router) = at.routers.get(&server) else {
            self.drop_copy(copy, displaced, at.now);
            return true;
        };
        on_node.held.state = OnServer::Unloading;
        let unloaded = |state| matches!(state, Status::Unloaded | Status::Failed);
        let done = router.ask("/models/unload", copy).await.is_ok()
            && matches!(
                router.wait(copy, unloaded, || false).await,
                Waited::Reached | Waited::Failed
            );
        if !done {
            if let Some(on_node) = self.copies.get_mut(&copy) {
                on_node.held.state = before;
            }
            return false;
        }
        self.drop_copy(copy, displaced, at.now);
        let _ = router.statuses(true).await;
        true
    }

    /// Frees the reservation of a copy that its server no longer holds, and writes the preset of the server
    /// again; a pinned copy that `displaced` names waits for its return.
    pub(super) fn drop_copy(&mut self, copy: Uuid, displaced: bool, now: u64) {
        let Some(on_node) = self.copies.remove(&copy) else {
            return;
        };
        let _ = self.write_preset(on_node.server);
        if displaced && on_node.held.keep.pinned {
            self.mark_displaced(copy, on_node.held.parts, now);
        }
    }

    /// Marks a pinned copy that lost its memory, so that it waits for its return; a mark soon after its last
    /// return waits longer.
    pub(super) fn mark_displaced(&mut self, copy: Uuid, parts: Vec<(Memory, u64)>, now: u64) {
        let Some(kept) = self.kept.get_mut(&copy) else {
            return;
        };
        let last = kept.returned_at.map(|at| (at, kept.interval));
        let mark = Displaced::new(copy, Load { parts }, last, now);
        kept.displaced = true;
        kept.interval = mark.interval;
        let _ = self.save(copy);
    }

    /// Loads `on_node` on its server, which the caller made room for. On each failure the daemon asks the router to
    /// unload the copy and to read the preset without it, so that no copy holds memory with no reservation.
    pub(super) async fn load_one(
        &mut self,
        at: &At<'_>,
        on_node: OnNode,
        by: &By<'_>,
    ) -> Result<(), Cause> {
        let (copy, server) = (on_node.held.copy, on_node.server);
        let router = at.routers.get(&server).ok_or(Cause::CopyUnknown)?;
        self.copies.insert(copy, on_node);
        let started = async {
            self.write_preset(server)?;
            router.statuses(true).await.ok_or(Cause::StartFailed)?;
            router.ask("/models/load", copy).await
        };
        let waited = match started.await {
            Ok(()) => {
                let stop = || matches!(by, By::Return(stop) if stop.load(Ordering::SeqCst));
                Ok(router
                    .wait(copy, |state| state == Status::Loaded, stop)
                    .await)
            }
            Err(cause) => Err(cause),
        };
        if waited == Ok(Waited::Reached) {
            if let Some(loaded) = self.copies.get_mut(&copy) {
                loaded.held.state = OnServer::Ready;
                loaded.held.used_at = super::now();
                loaded.decodes = router.usage(copy).await.map_or(0, |used| used.decodes);
            }
            return Ok(());
        }
        let _ = router.ask("/models/unload", copy).await;
        self.copies.remove(&copy);
        let _ = self.write_preset(server);
        let _ = router.statuses(true).await;
        Err(match waited {
            Ok(Waited::Failed) => Cause::LoadCrash,
            Ok(_) => Cause::StartFailed,
            Err(cause) => cause,
        })
    }

    /// The copy as it would hold memory on `server`.
    pub(super) fn on_node(&self, ask: &Ask<'_>, keep: Keep, now: u64) -> Result<OnNode, Cause> {
        let (model, projector) = self.files_of(ask.copy)?;
        let files_bytes = crate::model::pull::bytes_of(&self.folders.models, ask.copy);
        let reserved = reservation(files_bytes, ask.reserve_bytes, 0);
        // TODO(L1-5): the GPUs of the inventory; with none, a load is all on main memory.
        let parts = llama_parts(reserved, &[], GpuShare::All);
        Ok(OnNode {
            held: Held {
                copy: ask.copy,
                state: OnServer::Loading,
                reserved,
                parts,
                keep,
                used_at: now,
                returned_unused: false,
            },
            server: ask.server,
            model,
            projector,
            settings: ask.settings.clone(),
            decodes: 0,
        })
    }

    /// `model_load`: room by the rules of Section 11.1, then the load. A check that fails changes nothing: the room
    /// counts a copy that the load replaces as free, and the daemon unloads nothing before the room is known.
    ///
    /// # Errors
    /// The cause of Section 5.
    pub async fn load(&mut self, at: &At<'_>, ask: &Ask<'_>) -> Result<(), Cause> {
        if !at.routers.contains_key(&ask.server) {
            return Err(Cause::CopyUnknown);
        }
        let keep = ask
            .keep
            .or_else(|| self.kept.get(&ask.copy).map(|kept| kept.keep))
            .unwrap_or_default();
        let writes = keep != Keep::default() || self.kept.contains_key(&ask.copy);
        if writes && !self.kept.contains_key(&ask.copy) && self.kept.len() >= MAX_FILES {
            return Err(Cause::MemberUnknown);
        }
        // The use of each copy now, so that a copy with a request in progress is not taken for room.
        self.observe(at).await;
        let replaces = match self.copies.get(&ask.copy) {
            None => false,
            Some(there) => {
                let same = there.server == ask.server && there.settings == *ask.settings;
                match there.held.state {
                    OnServer::Ready | OnServer::Active if same => {
                        return self.kept_after(ask, keep, writes);
                    }
                    OnServer::Ready => true,
                    _ => return Err(Cause::MemoryBusy),
                }
            }
        };
        let on_node = self.on_node(ask, keep, at.now)?;
        let load = Load {
            parts: on_node.held.parts.clone(),
        };
        let others: Vec<Held> = self
            .held()
            .into_iter()
            .filter(|held| held.copy != ask.copy)
            .collect();
        let mut free: Vec<Uuid> = match room(at.usable, &others, &load, at.now) {
            Room::TooLarge => return Err(Cause::MemoryShare),
            Room::Busy => {
                for (memory, _) in &load.parts {
                    self.busy_at.insert(*memory, at.now);
                }
                return Err(Cause::MemoryBusy);
            }
            Room::Unload(copies) => copies,
            Room::Load => Vec::new(),
        };
        if replaces {
            free.insert(0, ask.copy);
        }
        for copy in free {
            // The copy that the load replaces is marked when pinned, so that it comes back if the load fails.
            if !self.unload_one(at, copy, true).await {
                return Err(Cause::MemoryBusy);
            }
        }
        self.load_one(at, on_node, &By::Order).await?;
        self.kept_after(ask, keep, writes)
    }

    /// The keep after a load: its keep, and, when the node keeps a file for the copy, its server, its settings and
    /// no mark.
    fn kept_after(&mut self, ask: &Ask<'_>, keep: Keep, writes: bool) -> Result<(), Cause> {
        if let Some(on_node) = self.copies.get_mut(&ask.copy) {
            on_node.held.keep = keep;
        }
        if !writes {
            return Ok(());
        }
        let kept = self.kept.entry(ask.copy).or_insert_with(|| Kept::new(keep));
        kept.keep = keep;
        kept.displaced = false;
        kept.server = Some(ask.server);
        kept.settings = settings::to_json(ask.settings);
        kept.reserve_bytes = ask.reserve_bytes;
        self.save(ask.copy)
    }
}
