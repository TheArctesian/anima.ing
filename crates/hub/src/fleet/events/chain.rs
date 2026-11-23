//! The audit log's hash chain (LOG-021 to LOG-028).

mod canonical;
mod check;
mod link;
#[cfg(test)]
mod tests;

use chrono::{DateTime, Days, Utc};
use sqlx::{PgConnection, PgPool};

pub(super) use check::{ChainCheck, Fault, FaultKind, Walk};
pub(super) use link::{Hash, Link, StoredEntry, hex};

use super::{
    Bus, Error, NewEvent, Severity, Source, actor::ActorKind, record, retention::Log,
    store::chain as store,
};

/// The chain's lock stays until the caller's transaction commits or rolls back (LOG-021, LOG-024).
pub(super) async fn append(
    conn: &mut PgConnection,
    entry: &StoredEntry,
) -> Result<(), sqlx::Error> {
    store::lock(conn).await?;
    let (seq, previous) = next(store::head(conn).await?);
    store::insert_links(conn, &[Link::new(seq, previous, entry)]).await
}

fn next(head: Option<(i64, Hash)>) -> (i64, Hash) {
    head.map_or((1, link::FIRST_PREVIOUS), |(seq, hash)| (seq + 1, hash))
}

/// Links every stored entry only when the chain is empty (LOG-026); call once at start, before the hub serves.
///
/// # Errors
/// `Error::Database` if an entry cannot be read or a link cannot be written; nothing is linked.
pub async fn link_existing(db: &PgPool) -> Result<u64, Error> {
    let mut tx = db.begin().await?;
    store::lock(&mut tx).await?;
    let mut head = store::head(&mut tx).await?;
    if head.is_some() {
        return Ok(0);
    }
    let mut linked = 0;
    let mut after = None;
    loop {
        let entries = store::entries_after(&mut tx, after).await?;
        let Some(last) = entries.last() else { break };
        after = Some((last.at, last.id));
        let links: Vec<Link> = entries
            .iter()
            .map(|entry| {
                let (seq, previous) = next(head);
                let link = Link::new(seq, previous, entry);
                head = Some((link.seq, link.hash));
                link
            })
            .collect();
        store::insert_links(&mut tx, &links).await?;
        linked += links.len() as u64;
    }
    tx.commit().await?;
    Ok(linked)
}

/// A link without its entry is a fault only inside the audit log's retention at `now` (LOG-027).
pub(super) async fn check_chain(db: &PgPool, now: DateTime<Utc>) -> Result<ChainCheck, Error> {
    // The audit log keeps one year whatever the event period is (DB-005).
    let mut walk = Walk::new(Log::Audit.first_kept_day(now, Days::new(0)));
    let unlinked = store::walk(db, &mut walk).await?;
    Ok(walk.finish(unlinked))
}

/// Writes a Hub Error event when the chain has a fault, and never repairs it (LOG-028).
///
/// # Errors
/// `Error::Database` if the check cannot run or the event cannot be written.
pub async fn check_chain_daily(db: &PgPool, bus: &Bus, now: DateTime<Utc>) -> Result<(), Error> {
    let Some(fault) = check_chain(db, now).await?.fault else {
        return Ok(());
    };
    let event = NewEvent {
        severity: Severity::Error,
        source: Source::Hub,
        device: None,
        message: fault_message(&fault),
        actor: ActorKind::Hub,
    };
    let mut conn = db.acquire().await?;
    record(&mut conn, bus, now, event).await.map(drop)
}

fn fault_message(fault: &Fault) -> String {
    let what = match fault.kind {
        FaultKind::Changed => "an entry was changed",
        FaultKind::Missing => "an entry was removed",
        FaultKind::Broken => "a link is missing or wrong",
        FaultKind::NotLinked => "an entry has no link",
    };
    let place = fault.seq.map_or_else(
        || format!("entry {}", fault.entry_id),
        |seq| format!("link {seq}"),
    );
    format!("The audit log chain is broken at {place}: {what}.")
}
