//! The orders that end a mark or free the copies of a server (K11 Section 11.1): `model_unload`, `model_keep`,
//! `copy_delete`, `server_start` and `server_stop`.

use daemon_core::model_work::cause::Cause;
use daemon_core::model_work::memory::keep::{Keep, Kept, MAX_FILES};
use daemon_core::model_work::settings;
use uuid::Uuid;

use super::{At, Loads, kept};

impl Loads {
    /// Ends the mark of a copy and forgets its server, so that it does not come back, also after a restart of the
    /// daemon, until a `model_load` loads it again.
    pub(super) fn end_mark(&mut self, copy: Uuid) {
        if let Some(kept) = self.kept.get_mut(&copy) {
            kept.displaced = false;
            kept.server = None;
        }
        let _ = self.save(copy);
    }

    /// `model_unload`: unloads the copy if `server` has it, and ends its mark.
    ///
    /// # Errors
    /// `copy_unknown` when the node does not have the server; `start_failed` when the router does not confirm the
    /// unload.
    pub async fn unload(&mut self, at: &At<'_>, server: Uuid, copy: Uuid) -> Result<(), Cause> {
        if !at.routers.contains_key(&server) {
            return Err(Cause::CopyUnknown);
        }
        let there = self
            .copies
            .get(&copy)
            .is_some_and(|on_node| on_node.server == server);
        if there && !self.unload_one(at, copy, false).await {
            return Err(Cause::StartFailed);
        }
        self.end_mark(copy);
        Ok(())
    }

    /// `model_keep`: the idle time and the pin of a copy on this node; a keep with no pin ends the mark. A copy that
    /// is loaded keeps its server and settings, so that it can come back when other work takes its memory.
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
            kept.server = Some(on_node.server);
            kept.settings = settings::to_json(&on_node.settings);
            kept.reserve_bytes = kept.reserve_bytes.or(Some(on_node.held.reserved));
        }
        self.save(copy)
    }

    /// `copy_delete`: unloads the copy and deletes its keep file.
    ///
    /// # Errors
    /// `start_failed` when the router does not confirm the unload; `limit_disk` when the state folder cannot be
    /// written.
    pub async fn forget(&mut self, at: &At<'_>, copy: Uuid) -> Result<(), Cause> {
        if !self.unload_one(at, copy, false).await {
            return Err(Cause::StartFailed);
        }
        self.kept.remove(&copy);
        kept::forget(&self.folders.state, copy)
    }

    /// `server_start`: the server starts again with no copy, so its copies free their reservations, and a pinned one
    /// waits for its return; a mark of an earlier run stays.
    pub fn server_started(&mut self, server: Uuid, now: u64) {
        let lost: Vec<Uuid> = self
            .copies
            .values()
            .filter(|copy| copy.server == server)
            .map(|copy| copy.held.copy)
            .collect();
        for copy in lost {
            self.drop_copy(copy, true, now);
        }
    }

    /// `server_stop`: the copies of the server free their reservations, and the marks of the copies of the server
    /// end.
    pub fn server_stopped(&mut self, server: Uuid) {
        self.copies.retain(|_, copy| copy.server != server);
        let marked: Vec<Uuid> = self
            .kept
            .iter()
            .filter(|(_, kept)| kept.server == Some(server))
            .map(|(copy, _)| *copy)
            .collect();
        for copy in marked {
            self.end_mark(copy);
        }
    }
}
