//! What the person accepts before a commit or a push (THR-049): who a commit is from, when
//! the repository sets it, and where a push goes. Both are in the configuration of the
//! repository, which an agent can write; the runner reads them as text and obeys nothing else
//! of that configuration. When one differs from what the person accepted, a commit and a push
//! are refused until the person accepts the change.
//!
//! Pure but for the file of what a person accepted for a repository.

use std::collections::BTreeMap;
use std::path::Path;

use agent_threads::git::GitRefusal;
use sha2::{Digest, Sha256};

use super::place::{Home, Worktree};
use super::run::hooks::Hooks;
use super::run::settings::{self, Setting};
use super::run::{Git, said};
use super::status;

/// The most names the page gets. More changes than these cannot be shown, so they cannot be
/// accepted.
pub const MOST_NAMES: usize = 40;

/// The most characters of one name that the page gets.
const MOST_CHARACTERS: usize = 300;

/// A value of the configuration as a part of a name for the page: one line, not too long.
fn shown(value: &str) -> String {
    value
        .chars()
        .map(|c| if c.is_control() { '?' } else { c })
        .take(MOST_CHARACTERS)
        .collect()
}

/// What a commit or a push of the person would take from the repository now: each as a name
/// that says it, with a digest of the whole value. `listed` is the configuration of the
/// repository as text.
#[must_use]
pub fn now(listed: &[Setting], hooks: Option<&Hooks<'_>>) -> BTreeMap<String, String> {
    let digest = |value: &str| crate::model::fetch::hex(&Sha256::digest(value.as_bytes()));
    // The hooks that a thread with full access runs are files that each thread can write.
    let mut seen = hooks.map_or_default(Hooks::entries);
    let who = settings::person(listed);
    if who.from_the_repository {
        let (name, email) = (who.name.unwrap_or_default(), who.email.unwrap_or_default());
        let whole = format!("{name}\0{email}");
        seen.insert(
            shown(&format!("commit as {name} <{email}>")),
            digest(&whole),
        );
    }
    if let Some((remote, url)) = status::remote(listed) {
        let place = said::destination(&url);
        seen.insert(
            format!("push to {remote}: {place}"),
            digest(&format!("pinned push\0{url}")),
        );
    }
    seen
}

/// [`now`] for the thread of `worktree`. A thread that runs no hook keeps the hooks as the
/// person accepted them before: none is shown to it as changed, and its yes accepts none.
#[must_use]
pub fn seen(
    worktree: &Worktree,
    listed: &[Setting],
    hooks: Option<&Hooks<'_>>,
) -> BTreeMap<String, String> {
    let mut seen = now(listed, hooks);
    if hooks.is_none()
        && let Some(before) = &worktree.record().trusted
    {
        let kept = before.iter().filter(|(name, _)| name.starts_with("hook "));
        seen.extend(kept.map(|(name, digest)| (name.clone(), digest.clone())));
    }
    seen
}

/// The names of what is not as the person accepted it: new, gone,
/// or with a different value: all of them, however many. A worktree with no accepted list has
/// each one as changed.
#[must_use]
pub fn changed(
    accepted: Option<&BTreeMap<String, String>>,
    now: &BTreeMap<String, String>,
) -> Vec<String> {
    let none = BTreeMap::new();
    let accepted = accepted.unwrap_or(&none);
    let differs = |name: &&String| accepted.get(*name) != now.get(*name);
    let mut names: Vec<&String> = now.keys().chain(accepted.keys()).filter(differs).collect();
    names.sort();
    names.dedup();
    names
        .into_iter()
        .map(|name| {
            if name.starts_with("push to ") {
                (*name).clone()
            } else {
                said::without_credentials(name)
            }
        })
        .collect()
}

/// Refuses when who a commit is from or where a push goes is not as the person accepted it
/// (THR-049). `listed` is the configuration of the repository as text.
///
/// # Errors
/// [`GitRefusal::Changed`] with the names of what changed.
pub fn accepted(
    worktree: &Worktree,
    listed: &[Setting],
    hooks: Option<&Hooks<'_>>,
) -> Result<(), GitRefusal> {
    let what = changed(
        worktree.record().trusted.as_ref(),
        &seen(worktree, listed, hooks),
    );
    if what.is_empty() {
        Ok(())
    } else {
        Err(GitRefusal::Changed { what: first(what) })
    }
}

/// The status with what changed since the person accepted it.
///
/// # Errors
/// As [`status::of`].
pub async fn status_with(
    git: &Git,
    worktree: &Worktree,
    listed: &[Setting],
    hooks: Option<&Hooks<'_>>,
) -> Result<agent_threads::git::Status, GitRefusal> {
    let mut status = status::of(git, worktree, listed).await?;
    let seen = seen(worktree, listed, hooks);
    status.changed = first(changed(worktree.record().trusted.as_ref(), &seen));
    status.accept = digest(&seen);
    Ok(status)
}

/// The names that the page gets: one more than can be shown says that there are more.
#[must_use]
pub fn first(mut names: Vec<String>) -> Vec<String> {
    names.truncate(MOST_NAMES + 1);
    names
}

/// One digest of a whole list: what the page shows the person and sends back with their yes.
#[must_use]
pub fn digest(entries: &BTreeMap<String, String>) -> String {
    let mut all = Sha256::new();
    for (name, value) in entries {
        all.update(name.as_bytes());
        all.update([0]);
        all.update(value.as_bytes());
        all.update(b"\n");
    }
    crate::model::fetch::hex(&all.finalize())
}

/// The file of what the person accepted for the repository at `project`, in the folder of the
/// records: outside the repository, where no agent of a thread writes.
fn baseline_file(home: &Home, project: &Path) -> std::path::PathBuf {
    let name = Sha256::digest(project.as_os_str().as_encoded_bytes());
    let name = crate::model::fetch::hex(&name);
    home.records().join("projects").join(format!("{name}.json"))
}

/// What the person last accepted for the repository, when an acceptance exists.
///
/// # Errors
/// [`GitRefusal::Failed`] when an existing record cannot be safely read or parsed.
pub fn baseline(
    home: &Home,
    project: &Path,
) -> Result<Option<BTreeMap<String, String>>, GitRefusal> {
    let refused = |error: std::io::Error| GitRefusal::Failed {
        said: format!("Could not read the accepted Git settings: {error}"),
    };
    let text = home
        .read_state(&baseline_file(home, project))
        .map_err(refused)?;
    text.map(|bytes| serde_json::from_slice(&bytes).map_err(|error| refused(error.into())))
        .transpose()
}

/// Writes what the person accepted for the repository at `project`.
///
/// # Errors
/// [`GitRefusal::Failed`] when the file cannot be written.
pub fn keep_baseline(
    home: &Home,
    project: &Path,
    accepted: &BTreeMap<String, String>,
) -> Result<(), GitRefusal> {
    let failed = || GitRefusal::Failed {
        said: "what you accepted could not be written down".to_owned(),
    };
    let file = baseline_file(home, project);
    let text = serde_json::to_vec_pretty(accepted).map_err(|_| failed())?;
    home.write_state(&file, &text).map_err(|_| failed())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn list(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
        pairs
            .iter()
            .map(|(name, digest)| ((*name).to_owned(), (*digest).to_owned()))
            .collect()
    }

    #[test]
    fn a_planted_author_and_each_place_of_a_push_are_named() {
        let read = |text: &str| settings::read(text);
        let own = "global\0user.name\nAda\0global\0user.email\nada@example.invalid\0";
        assert!(now(&read(own), None).is_empty());
        let planted = format!(
            "{own}local\0user.email\nm@example.invalid\n\u{1b}[2K\0\
             local\0remote.origin.url\nhttps://ada:TOKEN@example.invalid/a.git\0"
        );
        let seen = now(&read(&planted), None);
        let names: Vec<&str> = seen.keys().map(String::as_str).collect();
        assert_eq!(
            names,
            [
                "commit as Ada <m@example.invalid??[2K>",
                "push to origin: https://***@example.invalid/a.git"
            ]
        );
        let other = planted.replace("TOKEN", "OTHER");
        assert_ne!(
            now(&read(&other), None),
            seen,
            "a different credential is a change"
        );
    }

    #[test]
    fn a_new_a_gone_and_a_different_entry_are_each_named() {
        let accepted = list(&[("core.bare", "1"), ("hook pre-commit", "2"), ("gone", "3")]);
        let now = list(&[("core.bare", "1"), ("hook pre-commit", "9"), ("new", "4")]);
        assert_eq!(
            changed(Some(&accepted), &now),
            ["gone", "hook pre-commit", "new"]
        );
        assert_eq!(changed(Some(&now), &now), Vec::<String>::new());
        assert_eq!(changed(None, &now).len(), 3);
    }
}
