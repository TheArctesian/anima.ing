//! `anima agents files`: the files and folders of the working folder of a thread that the runner
//! started, the best match first (TH108), as the `@` list of the composer gives them. The runner
//! leaves out secrets, the folders of tools and of build output and what `.gitignore` leaves
//! out, and follows no link. `anima agents folders`: the folders in one folder of this computer
//! (TH113), to choose a project by browsing. Only the person asks either: the runner refuses them
//! from a process of an agent, so no MCP tool gives them.

use agent_threads::control::{Action, Outcome};
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
        invocation
            .output
            .message(&format!("No folder in {}.", folder.display()));
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
