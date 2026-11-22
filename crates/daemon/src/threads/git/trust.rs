//! What the person accepts before a commit or a push runs (THR-049, THR-050): the settings of
//! the repository and its hooks. An agent can write both, and a commit or a push of the person
//! runs the hooks and obeys the settings. The runner writes them down when it makes the
//! worktree, before an agent runs; when they differ later, a commit and a push are refused
//! until the person accepts the change.
//!
//! I/O: reads the folder of the hooks. [`changed`] is pure.

use std::collections::BTreeMap;
use std::path::Path;

use agent_threads::git::GitRefusal;
use sha2::{Digest, Sha256};

use super::place::{Home, Worktree};
use super::run::{Git, Run, said};

/// The most hooks and the most bytes of one hook that are read; more is refused.
const MOST_HOOKS: usize = 64;
const MOST_BYTES: u64 = 1024 * 1024;

/// The most names the page gets. More changes than these cannot be shown, so they cannot be
/// accepted; the count is taken after the hooks that do not run for the thread are left out.
pub const MOST_NAMES: usize = 40;

/// What a commit or a push of the person would run or obey now.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Seen {
    /// Each setting of the repository and each hook by name, with a digest.
    pub entries: BTreeMap<String, String>,
    /// The names of the hooks that can run.
    pub hooks: Vec<String>,
}

/// Reads the settings and the hooks of the repository of `worktree`; `git` is the one of
/// [`Git::for_worktree`].
///
/// # Errors
/// A [`GitRefusal`] when the folder of the hooks cannot be read or holds too much.
pub async fn now(git: &Git, worktree: &Worktree) -> Result<Seen, GitRefusal> {
    let mut seen = Seen {
        entries: git.of_the_repository().clone(),
        hooks: Vec::new(),
    };
    // Asked as a person whose hooks run: each other run has no folder of hooks at all.
    let asked = Run::new(worktree.folder(), ["rev-parse", "--git-path", "hooks"]);
    let with_hooks = git.clone().with_repository_programs(true);
    let folder = with_hooks.text(asked.for_the_person()).await?;
    let folder = worktree.folder().join(folder);
    for (name, bytes, runs) in hooks_in(&folder)? {
        let mut digest = Sha256::new();
        digest.update([u8::from(runs)]);
        digest.update(&bytes);
        seen.entries.insert(
            format!("{HOOK}{name}"),
            crate::model::fetch::hex(&digest.finalize()),
        );
        if runs {
            seen.hooks.push(name);
        }
    }
    seen.hooks.sort();
    Ok(seen)
}

/// Each file of the folder of the hooks that is not a sample: its name, its bytes, and
/// whether it can run. A folder that is not there has none.
fn hooks_in(folder: &Path) -> Result<Vec<(String, Vec<u8>, bool)>, GitRefusal> {
    let too_much = || GitRefusal::Failed {
        said: "the hooks of the repository are too many or too large to check".to_owned(),
    };
    let Ok(listed) = std::fs::read_dir(folder) else {
        return Ok(Vec::new());
    };
    let mut hooks = Vec::new();
    for entry in listed.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        // A link is followed, as git follows it.
        let Ok(about) = std::fs::metadata(entry.path()) else {
            continue;
        };
        if !about.is_file() || name.ends_with(".sample") {
            continue;
        }
        if hooks.len() == MOST_HOOKS || about.len() > MOST_BYTES {
            return Err(too_much());
        }
        let bytes = std::fs::read(entry.path()).map_err(|_| too_much())?;
        hooks.push((name, bytes, can_run(&about)));
    }
    Ok(hooks)
}

#[cfg(unix)]
fn can_run(about: &std::fs::Metadata) -> bool {
    use std::os::unix::fs::PermissionsExt as _;
    about.permissions().mode() & 0o111 != 0
}

#[cfg(not(unix))]
const fn can_run(_about: &std::fs::Metadata) -> bool {
    true
}

/// The start of the name of a hook in the list; the name of a setting has no space there.
pub const HOOK: &str = "hook ";

/// The names of the settings and hooks that are not as the person accepted them: new, gone,
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
        .map(|name| said::without_credentials(name))
        .collect()
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

/// What the person last accepted for the repository at `project`, in any thread. `None` when
/// they accepted nothing yet: a new worktree then starts with nothing accepted, so that what
/// an earlier agent left in the repository is shown before the first commit.
#[must_use]
pub fn baseline(home: &Home, project: &Path) -> Option<BTreeMap<String, String>> {
    let text = std::fs::read(baseline_file(home, project)).ok()?;
    serde_json::from_slice(&text).ok()
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
    let folder = file.parent().ok_or_else(failed)?;
    super::place::private_folder(folder).map_err(|_| failed())?;
    let text = serde_json::to_vec_pretty(accepted).map_err(|_| failed())?;
    std::fs::write(&file, text).map_err(|_| failed())
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
