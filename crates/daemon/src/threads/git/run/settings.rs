//! The configuration as `git config --list` gives it, and what the runner reads from it
//! (THR-050): the hooks that a run of the runner turns off, who the person is for a commit,
//! and where the remotes of the repository are. Each value is text to read; none is run.
//!
//! Pure: text in, lists out.

use super::{Asked, Git, Run};

/// The arguments that list each setting with the file it is from.
pub const LIST: [&str; 5] = ["config", "-z", "--show-scope", "--includes", "--list"];

/// One setting: where it is from, its name (git writes it in lower case but for a middle
/// part), and its value.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Setting {
    scope: String,
    name: String,
    value: String,
}

impl Setting {
    /// The setting is in the repository or its worktree, where an agent can write it.
    fn of_the_repository(&self) -> bool {
        matches!(self.scope.as_str(), "local" | "worktree")
    }
}

/// Reads the output of [`LIST`]: the scope, then the name and the value on two lines.
#[must_use]
pub fn read(listed: &str) -> Vec<Setting> {
    let mut settings = Vec::new();
    let mut fields = listed.split('\0');
    while let (Some(scope), Some(pair)) = (fields.next(), fields.next()) {
        let (name, value) = pair.split_once('\n').unwrap_or((pair, ""));
        settings.push(Setting {
            scope: scope.to_owned(),
            name: name.to_owned(),
            value: value.to_owned(),
        });
    }
    settings
}

/// Each hook that the configuration names (`hook.<name>.event`, `hook.<name>.command`; git
/// 2.54), from each file, with the setting that turns it off. `core.hooksPath` does not stop
/// these, and git runs them for `update-ref`, `add`, `restore`, `reset` and more.
#[must_use]
pub fn hooks_off(settings: &[Setting]) -> Vec<(String, String)> {
    let mut names: Vec<&str> = settings
        .iter()
        .filter_map(|setting| setting.name.strip_prefix("hook."))
        .filter_map(|rest| rest.rsplit_once('.'))
        .map(|(name, _)| name)
        .collect();
    names.sort_unstable();
    names.dedup();
    names
        .into_iter()
        .map(|name| (format!("hook.{name}.enabled"), "false".to_owned()))
        .collect()
}

/// The person signs their commits: `commit.gpgSign` is on outside the repository.
#[must_use]
pub fn signs(settings: &[Setting]) -> bool {
    settings
        .iter()
        .rfind(|setting| !setting.of_the_repository() && setting.name == "commit.gpgsign")
        .is_some_and(|setting| matches!(setting.value.as_str(), "" | "true" | "yes" | "on" | "1"))
}

/// The last value of `name`, as git takes it; with `own`, only outside the repository.
fn last<'a>(settings: &'a [Setting], name: &str, own: bool) -> Option<&'a Setting> {
    settings
        .iter()
        .rfind(|setting| setting.name == name && !(own && setting.of_the_repository()))
}

/// Who the person is for a commit, as their `git` has it in this repository.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Person {
    pub name: Option<String>,
    pub email: Option<String>,
    /// The name or the address is set in the repository, where an agent can write it.
    pub from_the_repository: bool,
    /// The key and the form of a signature, only as the person has them outside the
    /// repository: settings for a run of `git`.
    pub signing: Vec<(String, String)>,
}

/// Reads who the person is from the settings of their repository.
#[must_use]
pub fn person(settings: &[Setting]) -> Person {
    let (name, email) = (
        last(settings, "user.name", false),
        last(settings, "user.email", false),
    );
    let signing = ["user.signingkey", "gpg.format"]
        .into_iter()
        .filter_map(|key| last(settings, key, true))
        .map(|setting| (setting.name.clone(), setting.value.clone()))
        .collect();
    Person {
        from_the_repository: [name, email]
            .into_iter()
            .flatten()
            .any(Setting::of_the_repository),
        name: name.map(|setting| setting.value.clone()),
        email: email.map(|setting| setting.value.clone()),
        signing,
    }
}

/// Authentication settings Git may use with a pinned push destination. Repository settings
/// cannot supply credential helpers, and URL rewrite and remote settings are never replayed.
pub(in crate::threads::git) fn push_auth(
    settings: &[Setting],
) -> impl Iterator<Item = (&str, &str)> {
    settings
        .iter()
        .filter(|setting| {
            if !matches!(setting.scope.as_str(), "system" | "global") {
                return false;
            }
            let key = setting.name.as_str();
            key.starts_with("credential.")
                || (key.starts_with("http.")
                    && key.rsplit_once('.').is_some_and(|(_, name)| {
                        matches!(
                            name,
                            "extraheader"
                                | "cookiefile"
                                | "proxy"
                                | "proxyauthmethod"
                                | "sslcert"
                                | "sslkey"
                                | "sslcainfo"
                                | "sslcapath"
                                | "sslcertpasswordprotected"
                                | "pinnedpubkey"
                                | "emptyauth"
                                | "proactiveauth"
                        )
                    }))
        })
        .map(|setting| (setting.name.as_str(), setting.value.as_str()))
}

/// The folder of the hooks that the configuration names (`core.hooksPath`), when a file
/// names one. What the runner sets for its own run on the command line does not count.
#[must_use]
pub fn hooks_path(settings: &[Setting]) -> Option<&str> {
    settings
        .iter()
        .rfind(|setting| setting.name == "core.hookspath" && setting.scope != "command")
        .map(|setting| setting.value.as_str())
        .filter(|path| !path.is_empty())
}

/// The remote a push goes to: `remote.pushDefault`, else `origin`, else the first one, with
/// the URL of a push to it. `None` when the repository has no remote with a URL.
#[must_use]
pub fn push_remote(settings: &[Setting]) -> Option<(String, String)> {
    let part = |setting: &'_ Setting, key: &str| {
        let rest = setting.name.strip_prefix("remote.")?;
        let (name, found) = rest.rsplit_once('.')?;
        (found == key).then(|| name.to_owned())
    };
    let mut names: Vec<String> = settings
        .iter()
        .filter_map(|setting| part(setting, "url"))
        .collect();
    names.dedup();
    let set = last(settings, "remote.pushdefault", false).map(|setting| setting.value.as_str());
    let chosen = set
        .filter(|set| names.iter().any(|name| name == set))
        .or_else(|| {
            names
                .iter()
                .map(String::as_str)
                .find(|name| *name == "origin")
        })
        .or_else(|| names.first().map(String::as_str))?
        .to_owned();
    let url = ["pushurl", "url"].into_iter().find_map(|key| {
        settings
            .iter()
            .rfind(|setting| part(setting, key).as_deref() == Some(chosen.as_str()))
    })?;
    Some((chosen, url.value.clone()))
}

impl Git {
    /// The configuration a run gets through its environment, which wins over each file and
    /// can name a key with any character in it: each hook of the configuration, off, for a
    /// run of the runner itself; no push to a folder of this computer for the person.
    pub(super) fn overrides(&self, run: &Run<'_>) -> Vec<(String, String)> {
        let asked = run.asked;
        let mut set = match asked {
            Asked::Runner => self.hooks_off.clone(),
            Asked::Person => Vec::new(),
        };
        if asked == Asked::Person && !self.local_remotes {
            // A push to a folder runs the hooks of that folder, which nothing here turns off.
            set.push(("protocol.file.allow".to_owned(), "never".to_owned()));
        }
        set.extend(run.settings.iter().cloned());
        let mut env = vec![("GIT_CONFIG_COUNT".to_owned(), set.len().to_string())];
        for (at, (name, value)) in set.iter().enumerate() {
            env.push((format!("GIT_CONFIG_KEY_{at}"), name.clone()));
            env.push((format!("GIT_CONFIG_VALUE_{at}"), value.clone()));
        }
        env
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const LISTED: &str = "system\0credential.helper\nosxkeychain\0\
        global\0user.name\nAda\0\
        global\0user.email\nada@example.invalid\0\
        global\0user.signingkey\nKEY1\0\
        local\0core.bare\nfalse\0\
        local\0user.signingkey\nKEY2\0\
        worktree\0commit.gpgsign\ntrue\0\
        local\0hook.evil.event\nreference-transaction\0\
        local\0hook.evil.command\ntouch RAN\0\
        global\0hook.My Lint.command\nlint\0\
        global\0commit.gpgsign\ntrue\0\
        local\0remote.up.url\nhttps://example.invalid/up.git\0\
        local\0remote.origin.url\nhttps://example.invalid/a.git\0\
        local\0remote.origin.pushurl\nhttps://example.invalid/push.git\0\
        local\0branch.main.remote\norigin\0";

    #[test]
    fn each_hook_of_the_configuration_is_turned_off_by_name() {
        let off = hooks_off(&read(LISTED));
        let off: Vec<_> = off.iter().map(|(n, v)| (n.as_str(), v.as_str())).collect();
        assert_eq!(
            off,
            [
                ("hook.My Lint.enabled", "false"),
                ("hook.evil.enabled", "false")
            ]
        );
        assert!(signs(&read(LISTED)));
        assert!(!signs(&read("local\0commit.gpgsign\ntrue\0")));
    }

    #[test]
    fn the_person_is_read_with_the_key_that_only_they_set() {
        let who = person(&read(LISTED));
        assert_eq!(who.name.as_deref(), Some("Ada"));
        assert!(!who.from_the_repository);
        assert_eq!(
            who.signing,
            [("user.signingkey".to_owned(), "KEY1".to_owned())]
        );
        let planted = format!("{LISTED}local\0user.email\nmallory@example.invalid\0");
        let who = person(&read(&planted));
        assert_eq!(who.email.as_deref(), Some("mallory@example.invalid"));
        assert!(who.from_the_repository);
    }

    #[test]
    fn the_folder_of_the_hooks_is_what_a_file_says_not_what_the_runner_set_for_its_run() {
        let listed = "local\0core.hookspath\n.husky/_\0command\0core.hookspath\n/dev/null\0";
        assert_eq!(hooks_path(&read(listed)), Some(".husky/_"));
        assert_eq!(
            hooks_path(&read("command\0core.hookspath\n/dev/null\0")),
            None
        );
    }

    #[test]
    fn a_push_goes_to_origin_at_its_push_url_unless_the_person_set_a_default() {
        let found = push_remote(&read(LISTED));
        let want = (
            "origin".to_owned(),
            "https://example.invalid/push.git".to_owned(),
        );
        assert_eq!(found, Some(want));
        let set = format!("{LISTED}global\0remote.pushdefault\nup\0");
        let found = push_remote(&read(&set)).map(|(name, _)| name);
        assert_eq!(found.as_deref(), Some("up"));
        assert_eq!(push_remote(&read("local\0core.bare\nfalse\0")), None);
    }
}
