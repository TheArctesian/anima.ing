//! `anima agents files`: the files and folders of the working folder of a thread that the runner
//! started, the best match first (TH108), as the `@` list of the composer gives them. The runner
//! leaves out secrets, the folders of tools and of build output and what `.gitignore` leaves
//! out, and follows no link. Only the person asks it: the runner refuses it from a process of an
//! agent, so no MCP tool gives it.

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
