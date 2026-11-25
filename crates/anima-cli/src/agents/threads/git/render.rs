use std::fmt::Write as _;

use agent_threads::git::{ChangedFile, Changes, GitOutcome, Status};

use crate::failure::Failure;
use crate::output::{
    Cell, Output, Table,
    text::{clean, enum_name},
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
                visible_text(url)
            ))
        }
        GitOutcome::PullRequest(page) => output.line(&clean(&page.url)),
        GitOutcome::Removed {
            kept,
            recovery,
            original,
        } => show_removed(
            (kept.as_deref(), recovery.as_deref(), original.as_deref()),
            output,
        ),
        GitOutcome::Trusted => output.line("Git settings accepted."),
        GitOutcome::Refused { why } => Err(super::refused(why.clone())),
    }
}

fn show_removed(
    (kept, recovery, original): (
        Option<&str>,
        Option<&std::path::Path>,
        Option<&std::path::Path>,
    ),
    output: Output,
) -> Result<(), Failure> {
    let kept = kept.map_or_else(String::new, |reference| {
        format!(" Git snapshot: {}.", clean(reference))
    });
    if let Some(recovery) = recovery {
        let recovery = recovery.display().to_string();
        output.line(&format!(
            "Worktree retired.{kept} Complete directory: {}",
            recovery.escape_debug()
        ))?;
        if let Some(original) = original {
            output.line(&format!(
                "Original directory: {}. Resolve relative nested Git links from their original locations. Use --git-dir and --work-tree to inspect recovered submodules.",
                original.display().to_string().escape_debug()
            ))?;
        }
        Ok(())
    } else if let Some(original) = original {
        output.line(&format!(
            "Cleared the worktree registration. Former path: {}. No recovery directory was created. An existing branch stays in the repository.",
            original.display().to_string().escape_debug()
        ))
    } else {
        output.line(&format!("Worktree removed.{kept}"))
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
        output.line(&format!("Push URL: {}", visible_text(url)))?;
    }
    show_files(&status.files, output)?;
    if !status.changed.is_empty() {
        output.line("Review these Git settings before accepting them:")?;
        for line in &status.changed {
            output.line(&format!("  {}", visible_text(line)))?;
        }
        output.line(&format!("Acceptance digest: {}", clean(&status.accept)))?;
    }
    Ok(())
}

fn show_changes(changes: &Changes, output: Output) -> Result<(), Failure> {
    show_files(&changes.files, output)?;
    if changes.truncated {
        output.line("Patches could not be displayed in full.")?;
    }
    if changes.files.iter().any(|file| file.binary) {
        output.line("Binary changes require inspection outside this commit review.")?;
    }
    for patch in changes
        .files
        .iter()
        .filter_map(|file| file.patch.as_deref())
    {
        output.line(visible(patch.trim_end_matches('\n'), true).as_str())?;
    }
    if let Some(snapshot) = &changes.snapshot {
        output.line(&format!("Reviewed snapshot: {}", clean(snapshot)))?;
    }
    Ok(())
}

/// Keep Git's line structure. Escape non-ASCII and controls, including tabs, so invisible
/// characters and literal text that looks like an escape remain distinguishable.
pub(super) fn visible_text(text: &str) -> String {
    visible(text, false)
}

fn visible(patch: &str, multiline: bool) -> String {
    let mut visible = String::with_capacity(patch.len());
    for character in patch.chars() {
        match character {
            '\\' => visible.push_str("\\\\"),
            '\n' if multiline => visible.push(character),
            ' '..='~' => visible.push(character),
            other => {
                let _ = write!(visible, "\\u{{{:x}}}", u32::from(other));
            }
        }
    }
    visible
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
