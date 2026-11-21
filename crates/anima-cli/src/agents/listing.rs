//! `anima agents pin`, `archive`, `rename`, `remove` and `listings`: how the person lists the
//! threads of this computer (TH46). The runner keeps it in its own record; no session file of
//! Claude Code or Codex changes. No MCP tool does it: the runner refuses it from a process of
//! an agent, so that an agent cannot hide a thread from the person.

use agent_threads::control::{Action, Listing, ListingChange, Outcome};
use uuid::Uuid;

use super::drive::accepted;
use super::runner::RunnerLink;
use crate::failure::Failure;
use crate::invocation::Invocation;
use crate::output::{Cell, Table};

#[derive(Debug, clap::Args)]
pub struct SwitchArgs {
    thread: Uuid,
    /// Take the mark away.
    #[arg(long)]
    off: bool,
}

#[derive(Debug, clap::Args)]
pub struct RenameArgs {
    thread: Uuid,
    /// The new title. With none, the thread takes the title of its program again.
    title: Option<String>,
}

#[derive(Debug, clap::Args)]
pub struct RemoveArgs {
    thread: Uuid,
}

/// The question before a thread is removed from the list.
pub const REMOVE: &str = "Remove this thread from the list of Anima? Its pin, archive and title go too. The session files of Claude Code or Codex stay on disk.";

async fn change(thread: Uuid, change: ListingChange) -> Result<(), Failure> {
    let mut link = RunnerLink::connect().await?;
    accepted(link.call(Action::ChangeListing { thread, change }).await?)?;
    Ok(())
}

/// `anima agents pin`: the thread is listed first.
///
/// # Errors
/// [`Failure`] for a refusal of the runner, or a runner that is not there.
pub async fn pin(args: SwitchArgs, invocation: &Invocation) -> Result<(), Failure> {
    change(args.thread, ListingChange::Pin { on: !args.off }).await?;
    invocation
        .output
        .message(if args.off { "Not pinned." } else { "Pinned." });
    Ok(())
}

/// `anima agents archive`: the thread is listed only with the archived threads.
///
/// # Errors
/// [`Failure`] for a refusal of the runner, or a runner that is not there.
pub async fn archive(args: SwitchArgs, invocation: &Invocation) -> Result<(), Failure> {
    change(args.thread, ListingChange::Archive { on: !args.off }).await?;
    invocation.output.message(if args.off {
        "Not archived."
    } else {
        "Archived."
    });
    Ok(())
}

/// `anima agents rename`: a title of the person's own, or the title of the program again.
///
/// # Errors
/// [`Failure`] for a refusal of the runner (a title that is empty, too long or has hidden
/// marks), or a runner that is not there.
pub async fn rename(args: RenameArgs, invocation: &Invocation) -> Result<(), Failure> {
    let cleared = args.title.is_none();
    change(args.thread, ListingChange::Rename { title: args.title }).await?;
    invocation.output.message(if cleared {
        "The thread has the title of its program again."
    } else {
        "Renamed."
    });
    Ok(())
}

/// `anima agents remove`: the thread leaves the list of Anima, after the question. The runner
/// refuses a thread that runs.
///
/// # Errors
/// [`Failure::Declined`] for a no; a refusal of the runner.
pub async fn remove(args: RemoveArgs, invocation: &Invocation) -> Result<(), Failure> {
    invocation.prompter.confirm(REMOVE)?;
    change(args.thread, ListingChange::Remove).await?;
    invocation
        .output
        .message("Removed from the list. The session files stay on disk.");
    Ok(())
}

/// `anima agents listings`: each thread that is pinned, archived, renamed or removed.
///
/// # Errors
/// [`Failure`] when the runner is not there.
pub async fn listings(invocation: &Invocation) -> Result<(), Failure> {
    let mut link = RunnerLink::connect().await?;
    let Outcome::Listings { listings } = accepted(link.call(Action::Listings).await?)? else {
        return Ok(());
    };
    invocation
        .output
        .data(&listings, |listings| table(listings))?;
    Ok(())
}

fn table(listings: &[Listing]) -> Table {
    let mut table = Table::new(vec!["THREAD", "LISTED", "TITLE"]);
    for listing in listings {
        let listed = [
            (listing.pinned, "pinned"),
            (listing.archived, "archived"),
            (listing.removed, "removed"),
        ]
        .iter()
        .filter(|(on, _)| *on)
        .map(|(_, word)| *word)
        .collect::<Vec<_>>()
        .join(", ");
        table.row(vec![
            Cell::new(&listing.thread.to_string()),
            Cell::new(&listed),
            Cell::new(listing.title.as_deref().unwrap_or("")),
        ]);
    }
    table
}
