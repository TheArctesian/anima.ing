use agent_threads::git::{ChangedFile, Changes, GitOutcome, Status};

use crate::failure::Failure;
use crate::output::{
    Cell, Output, Table,
    text::{clean, clean_file, enum_name},
};

pub(super) fn show(result: &GitOutcome, output: Output) -> Result<(), Failure> {
    if output.json() {
        let value = serde_json::to_value(result).map_err(|error| Failure::Write {
            reason: error.to_string(),
        })?;
        return output.raw(&value);
    }
    match result {
        GitOutcome::Status(status) => show_status(status, output),
        GitOutcome::Checkpoints { checkpoints } => output.data(checkpoints, |rows| {
            let mut table = Table::new(vec!["TURN", "COMMIT", "UNIX TIME"]);
            for checkpoint in rows {
                table.row(vec![
                    Cell::new(&checkpoint.turn.to_string()),
                    Cell::new(&checkpoint.commit),
                    Cell::new(&checkpoint.at.to_string()),
                ]);
            }
            table
        }),
        GitOutcome::Changes(changes) => show_changes(changes, output),
        GitOutcome::Reverted { turn, undo } => {
            output.line(&format!("Restored turn {turn}. Undo: {}", clean(undo)))
        }
        GitOutcome::Committed { commit, branch } => {
            let branch = branch
                .as_deref()
                .map_or_else(String::new, |branch| format!(" on {}", clean(branch)));
            output.line(&format!("Committed {}{branch}", clean(commit)))
        }
        GitOutcome::Pushed {
            remote,
            branch,
            url,
            up_to_date,
        } => {
            let verb = if *up_to_date { "Up to date" } else { "Pushed" };
            output.line(&format!(
                "{verb}: {} at {} ({})",
                clean(branch),
                clean(remote),
                clean(url)
            ))
        }
        GitOutcome::PullRequest(page) => output.line(&clean(&page.url)),
        GitOutcome::Removed { kept } => {
            let kept = kept.as_deref().map_or_else(String::new, |reference| {
                format!(" Work kept at {}.", clean(reference))
            });
            output.line(&format!("Worktree removed.{kept}"))
        }
        GitOutcome::Trusted => output.line("Git settings accepted."),
        GitOutcome::Refused { why } => Err(super::refused(why.clone())),
    }
}

fn show_status(status: &Status, output: Output) -> Result<(), Failure> {
    let branch = status.branch.as_deref().unwrap_or("(detached HEAD)");
    let upstream = status.upstream.as_deref().unwrap_or("(none)");
    output.line(&format!(
        "Branch: {}\nUpstream: {}\nAhead: {}  Behind: {}  Ahead of default: {}",
        clean(branch),
        clean(upstream),
        status.ahead,
        status.behind,
        status.ahead_of_default
    ))?;
    if let Some(url) = &status.push_url {
        output.line(&format!("Push URL: {}", clean(url)))?;
    }
    show_files(&status.files, output)?;
    if !status.changed.is_empty() {
        output.line("Review these Git settings before accepting them:")?;
        for line in &status.changed {
            output.line(&format!("  {}", clean(line)))?;
        }
        output.line(&format!("Acceptance digest: {}", clean(&status.accept)))?;
    }
    Ok(())
}

fn show_changes(changes: &Changes, output: Output) -> Result<(), Failure> {
    show_files(&changes.files, output)?;
    if changes.truncated {
        output.line("Patches exceed the display limit.")?;
    }
    for patch in changes
        .files
        .iter()
        .filter_map(|file| file.patch.as_deref())
    {
        output.line(clean_file(patch).trim_end_matches('\n'))?;
    }
    if let Some(snapshot) = &changes.snapshot {
        output.line(&format!("Reviewed snapshot: {}", clean(snapshot)))?;
    }
    Ok(())
}

fn show_files(files: &[ChangedFile], output: Output) -> Result<(), Failure> {
    if files.is_empty() {
        return output.line("No changed files.");
    }
    output.data(&files, |files| {
        let mut table = Table::new(vec!["CHANGE", "FILE", "+", "-"]);
        for file in *files {
            let path = file.old_path.as_ref().map_or_else(
                || file.path.clone(),
                |old| format!("{old} -> {}", file.path),
            );
            table.row(vec![
                Cell::new(&enum_name(&file.kind)),
                Cell::new(&path),
                Cell::new(&file.additions.to_string()),
                Cell::new(&file.deletions.to_string()),
            ]);
        }
        table
    })
}
