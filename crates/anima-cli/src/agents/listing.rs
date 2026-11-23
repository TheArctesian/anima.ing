//! How the person lists threads (TH46, TH61) and projects (TH107). The runner refuses these from
//! an agent's process, so that an agent cannot hide a thread; no MCP tool gives them.

use std::collections::BTreeSet;

use agent_threads::control::{
    Action, Listing, ListingChange, MAX_TITLE, MOST_SNOOZE, Outcome, Project, ProjectChange,
};
use chrono::{DateTime, TimeDelta, Utc};
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

#[derive(Debug, clap::Args)]
pub struct WakeArgs {
    thread: Uuid,
}

#[derive(Debug, clap::Args)]
#[command(group(clap::ArgGroup::new("end").required(true)))]
pub struct SnoozeArgs {
    thread: Uuid,
    /// How long, as a number and a unit: `90m`, `3h`, `2d`.
    #[arg(long = "for", value_parser = snooze_for, group = "end")]
    length: Option<TimeDelta>,
    /// The end of the snooze, as RFC 3339 (`2026-11-23T09:00:00+01:00`).
    #[arg(long, group = "end")]
    until: Option<DateTime<Utc>>,
}

#[derive(Debug, clap::Args)]
pub struct ProjectsArgs {
    #[command(subcommand)]
    change: Option<ProjectCommand>,
}

#[derive(Debug, clap::Subcommand)]
enum ProjectCommand {
    /// Add a folder of this computer as a project.
    Add {
        folder: std::path::PathBuf,
        /// The name of the project; by default the last part of the folder.
        #[arg(long)]
        name: Option<String>,
    },
    /// Give a project a new name.
    Rename {
        folder: std::path::PathBuf,
        name: String,
    },
    /// Take a project off the list. No file and no thread of it changes.
    Remove { folder: std::path::PathBuf },
}

/// `anima agents projects`: lists the projects, after the change that the arguments name.
///
/// # Errors
/// [`Failure`] for a refusal of the runner, or a runner that is not there.
pub async fn projects(args: ProjectsArgs, invocation: &Invocation) -> Result<(), Failure> {
    let absolute = |folder: std::path::PathBuf| std::path::absolute(&folder).unwrap_or(folder);
    let action = match args.change {
        None => Action::Projects,
        Some(command) => Action::ChangeProject {
            change: match command {
                ProjectCommand::Add { folder, name } => ProjectChange::Add {
                    folder: absolute(folder),
                    name,
                },
                ProjectCommand::Rename { folder, name } => ProjectChange::Rename {
                    folder: absolute(folder),
                    name,
                },
                ProjectCommand::Remove { folder } => ProjectChange::Remove {
                    folder: absolute(folder),
                },
            },
        },
    };
    let mut link = RunnerLink::connect().await?;
    let Outcome::Projects { projects } = accepted(link.call(action).await?)? else {
        return Ok(());
    };
    if projects.is_empty() {
        invocation.output.message("No projects.");
        return Ok(());
    }
    invocation
        .output
        .data(&projects, |rows| project_table(rows))
}

fn project_table(rows: &[Project]) -> Table {
    let mut table = Table::new(vec!["NAME", "FOLDER"]);
    for row in rows {
        table.row(vec![
            Cell::new(&row.name),
            Cell::new(&row.folder.display().to_string()),
        ]);
    }
    table
}

/// The question before a thread is removed from the list.
pub const REMOVE: &str = "Remove this thread from Anima's list? Its pin, archive and title go too. The session files of Claude Code or Codex stay on disk.";

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
    let rename = ListingChange::Rename { title: args.title };
    // The same check as the runner's, here first, so that the words are about a title.
    if Listing::default()
        .changed(&rename, chrono::Utc::now())
        .is_err()
    {
        return Err(Failure::Usage {
            message: format!(
                "A title takes 1 to {MAX_TITLE} characters that show: no control characters and no hidden marks, such as a mark that turns the direction of text."
            ),
            hint: None,
        });
    }
    change(args.thread, rename).await?;
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

/// `anima agents settle`: the thread goes to the shelf Settled; `--off` takes it back.
///
/// # Errors
/// [`Failure`] for a refusal of the runner, or a runner that is not there.
pub async fn settle(args: SwitchArgs, invocation: &Invocation) -> Result<(), Failure> {
    change(args.thread, ListingChange::Settle { on: !args.off }).await?;
    invocation.output.message(if args.off {
        "Back with the active threads."
    } else {
        "Settled."
    });
    Ok(())
}

/// `anima agents snooze`: the thread waits on the shelf Snoozed until a time, at most 30 days
/// away. A prompt, or a request of the program, wakes it before.
///
/// # Errors
/// [`Failure::Usage`] for a time that is not ahead or too far; a refusal of the runner.
pub async fn snooze(args: SnoozeArgs, invocation: &Invocation) -> Result<(), Failure> {
    let now = Utc::now();
    let until = match (args.length, args.until) {
        (Some(length), _) => now + length,
        (None, Some(until)) => until,
        (None, None) => unreachable!("clap asks for --for or --until"),
    };
    if until <= now || until - now > MOST_SNOOZE {
        return Err(Failure::Usage {
            message: "A snooze ends after now and at most 30 days from now.".to_owned(),
            hint: None,
        });
    }
    change(args.thread, ListingChange::Snooze { until }).await?;
    invocation
        .output
        .message(&format!("Snoozed until {}.", until.to_rfc3339()));
    Ok(())
}

/// `anima agents wake`: ends the snooze of the thread now.
///
/// # Errors
/// [`Failure`] for a refusal of the runner, or a runner that is not there.
pub async fn wake(args: WakeArgs, invocation: &Invocation) -> Result<(), Failure> {
    change(args.thread, ListingChange::Wake).await?;
    invocation.output.message("Awake.");
    Ok(())
}

/// The length of a snooze from `90m`, `3h` or `2d`, at most 30 days.
fn snooze_for(text: &str) -> Result<TimeDelta, String> {
    let unit = text.chars().last().unwrap_or(' ');
    let number = &text[..text.len() - unit.len_utf8().min(text.len())];
    let count: i64 = number
        .parse()
        .ok()
        .filter(|count| *count > 0)
        .ok_or_else(|| format!("{text:?} is not a number and a unit, such as 3h"))?;
    let length = match unit {
        'm' => TimeDelta::try_minutes(count),
        'h' => TimeDelta::try_hours(count),
        'd' => TimeDelta::try_days(count),
        _ => return Err(format!("{text:?}: the unit is m, h or d")),
    };
    length
        .filter(|length| *length <= MOST_SNOOZE)
        .ok_or_else(|| format!("{text:?}: a snooze is at most 30 days"))
}

/// `anima agents listings`: each thread that is pinned, archived, renamed, removed, settled or
/// snoozed.
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

/// The IDs of the threads that the person removed from the list, from the runner.
///
/// # Errors
/// [`Failure`] when the runner is not there or refuses.
pub async fn removed() -> Result<BTreeSet<String>, Failure> {
    let mut link = RunnerLink::connect().await?;
    let Outcome::Listings { listings } = accepted(link.call(Action::Listings).await?)? else {
        return Ok(BTreeSet::new());
    };
    Ok(listings
        .iter()
        .filter(|listing| listing.removed)
        .map(|listing| listing.thread.to_string())
        .collect())
}

fn table(listings: &[Listing]) -> Table {
    let mut table = Table::new(vec!["THREAD", "LISTED", "TITLE"]);
    for listing in listings {
        let snoozed = listing
            .marks
            .snoozed_until
            .filter(|until| *until > Utc::now())
            .map(|until| format!("snoozed until {}", until.to_rfc3339()));
        let listed = [
            (listing.pinned, "pinned"),
            (listing.archived, "archived"),
            (listing.removed, "removed"),
            (listing.marks.settled_at.is_some(), "settled"),
        ]
        .iter()
        .filter(|(on, _)| *on)
        .map(|(_, word)| (*word).to_owned())
        .chain(snoozed)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn th61_a_snooze_length_is_a_number_and_a_unit() {
        assert_eq!(snooze_for("90m"), Ok(TimeDelta::minutes(90)));
        assert_eq!(snooze_for("3h"), Ok(TimeDelta::hours(3)));
        assert_eq!(snooze_for("2d"), Ok(TimeDelta::days(2)));
        assert_eq!(snooze_for("30d"), Ok(MOST_SNOOZE));
        for bad in [
            "",
            "h",
            "0h",
            "-1h",
            "3",
            "3w",
            "3 h",
            "31d",
            "99999999999999d",
            "3é",
        ] {
            assert!(snooze_for(bad).is_err(), "{bad:?}");
        }
    }
}
