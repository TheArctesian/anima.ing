//! `anima agents files` (TH108), `anima agents folders` (TH113) and `anima agents commands`
//! (TH117): the lists of the composer. The runner refuses each from an agent's process, so no
//! MCP tool gives them.

use agent_threads::control::{Action, Outcome};
use agent_threads::drive::CommandLists;
use agent_threads::files::FoundPath;
use uuid::Uuid;

use super::drive::accepted;
use super::runner::RunnerLink;
use crate::failure::Failure;
use crate::invocation::Invocation;
use crate::output::{Cell, Table};

#[derive(Debug, clap::Args)]
pub struct FilesArgs {
    thread: Uuid,
    /// Words of a name or a path; with none, the top of the folder.
    #[arg(default_value = "")]
    query: String,
    /// The most paths to show.
    #[arg(long, default_value_t = 50, value_parser = clap::value_parser!(u16).range(1..=200))]
    most: u16,
}

/// `anima agents files`.
///
/// # Errors
/// [`Failure`] for a refusal of the runner (a thread it did not start, a query that is too long),
/// or a runner that is not there.
pub async fn files(args: FilesArgs, invocation: &Invocation) -> Result<(), Failure> {
    let mut link = RunnerLink::connect().await?;
    let action = Action::Files {
        thread: args.thread,
        query: args.query,
        most: usize::from(args.most),
    };
    let Outcome::Files { paths, more } = accepted(link.call(action).await?)? else {
        return Ok(());
    };
    if paths.is_empty() {
        invocation.output.message("No file matches.");
        return Ok(());
    }
    invocation.output.data(&paths, |rows| table(rows))?;
    if more {
        invocation
            .output
            .message("More files match: give more words, or a larger --most.");
    }
    Ok(())
}

fn table(rows: &[FoundPath]) -> Table {
    let mut table = Table::new(vec!["KIND", "PATH"]);
    for row in rows {
        let kind = if row.folder { "folder" } else { "file" };
        table.row(vec![Cell::new(kind), Cell::new(&row.path)]);
    }
    table
}

#[derive(Debug, clap::Args)]
pub struct FoldersArgs {
    /// An absolute folder, or a relative one from here; with none, the folder here.
    #[arg(default_value = ".")]
    folder: std::path::PathBuf,
    /// Show the hidden folders too (a name that starts with a dot).
    #[arg(long)]
    hidden: bool,
}

/// `anima agents folders`.
///
/// # Errors
/// [`Failure`] for a refusal of the runner (no such folder, or the folder of Anima), or a runner
/// that is not there.
pub async fn folders(args: FoldersArgs, invocation: &Invocation) -> Result<(), Failure> {
    let mut link = RunnerLink::connect().await?;
    let action = Action::Folders {
        folder: std::path::absolute(&args.folder).unwrap_or(args.folder),
        hidden: args.hidden,
    };
    let Outcome::Folders {
        folder,
        folders,
        more,
    } = accepted(link.call(action).await?)?
    else {
        return Ok(());
    };
    if folders.is_empty() {
        invocation.output.message(&format!(
            "No folder in {}.",
            folder.display().to_string().escape_debug()
        ));
        return Ok(());
    }
    invocation.output.data(&folders, |rows| {
        let mut table = Table::new(vec!["FOLDER"]);
        for name in rows {
            table.row(vec![Cell::new(name)]);
        }
        table
    })?;
    if more {
        invocation
            .output
            .message("The folder holds more folders than are shown.");
    }
    Ok(())
}

#[derive(Debug, clap::Args)]
#[command(group = clap::ArgGroup::new("of").required(true).args(["thread", "program"]))]
pub struct CommandsArgs {
    /// A thread that the runner started.
    #[arg(long, conflicts_with = "folder")]
    thread: Option<Uuid>,
    /// A program, with the folder of a new thread.
    #[arg(long, value_enum, requires = "folder")]
    program: Option<super::drive::ProgramName>,
    /// An absolute folder, or a relative one from here.
    #[arg(long)]
    folder: Option<std::path::PathBuf>,
}

/// `anima agents commands`: the commands and the skills of a program, as the `/` and `$` lists
/// of the composer show them.
///
/// # Errors
/// [`Failure`] for a refusal of the runner, or a runner that is not there.
pub async fn commands(args: CommandsArgs, invocation: &Invocation) -> Result<(), Failure> {
    let mut link = RunnerLink::connect().await?;
    let action = Action::Commands {
        thread: args.thread,
        program: args.program.map(super::drive::ProgramName::program),
        folder: args
            .folder
            .map(|folder| std::path::absolute(&folder).unwrap_or(folder)),
    };
    let Outcome::Commands { lists, pending } = accepted(link.call(action).await?)? else {
        return Ok(());
    };
    if pending {
        invocation
            .output
            .message("The program has not named its commands yet: ask again soon.");
        return Ok(());
    }
    invocation.output.data(&lists, |lists| command_table(lists))
}

fn command_table(lists: &CommandLists) -> Table {
    let mut table = Table::new(vec!["KIND", "NAME", "DESCRIPTION"]);
    for command in &lists.commands {
        let words = command.description.as_deref().unwrap_or("");
        table.row(vec![
            Cell::new("command"),
            Cell::new(&command.name),
            Cell::new(words),
        ]);
    }
    for skill in &lists.skills {
        let words = skill.short.as_deref().or(skill.description.as_deref());
        table.row(vec![
            Cell::new("skill"),
            Cell::new(&skill.name),
            Cell::new(words.unwrap_or("")),
        ]);
    }
    table
}
