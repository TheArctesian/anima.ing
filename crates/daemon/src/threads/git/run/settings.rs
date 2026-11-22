//! The configuration of a repository as `git config --list` gives it, and what a run does with
//! it (THR-050): the filters that a run of the runner turns off, the settings that a run for
//! the person takes from the person and never from the repository, and the settings of the
//! repository that the person accepts before a commit or a push.
//!
//! Pure: text in, lists out.

use std::collections::BTreeMap;

use sha2::{Digest, Sha256};

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

/// The settings that name a program or decide whom a push trusts, each with what git does
/// when nobody set it. A run for the person takes them from the person, never from the
/// repository, which an agent can write.
const OF_THE_PERSON: [(&str, &str); 12] = [
    ("core.askpass", ""),
    ("gpg.program", "gpg"),
    ("gpg.openpgp.program", "gpg"),
    ("gpg.format", "openpgp"),
    ("gpg.ssh.program", "ssh-keygen"),
    ("gpg.x509.program", "gpgsm"),
    ("commit.gpgsign", "false"),
    ("http.proxy", ""),
    ("http.sslverify", "true"),
    ("http.cookiefile", ""),
    ("push.gpgsign", "false"),
    ("gpg.ssh.defaultkeycommand", ""),
];

impl Setting {
    /// The setting is in the repository or its worktree, where an agent can write it.
    fn of_the_repository(&self) -> bool {
        matches!(self.scope.as_str(), "local" | "worktree")
    }

    fn names_a_helper(&self) -> bool {
        self.name.starts_with("credential.") && self.name.ends_with(".helper")
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

/// Each filter setting of the repository with the value that turns it off.
#[must_use]
pub fn filters_off(settings: &[Setting]) -> Vec<(String, String)> {
    settings
        .iter()
        .filter(|setting| setting.of_the_repository() && setting.name.starts_with("filter."))
        .map(|setting| {
            let off = if setting.name.ends_with(".required") {
                "false"
            } else {
                ""
            };
            (setting.name.clone(), off.to_owned())
        })
        .collect()
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

/// What a run for the person sets after each file: no push option and no `git://` (so no
/// proxy program), and each setting of [`OF_THE_PERSON`] and each credential helper as the
/// person has it, when the repository names one.
#[must_use]
pub fn for_the_person(settings: &[Setting]) -> Vec<(String, String)> {
    let mut over = vec![
        ("push.pushOption".to_owned(), String::new()),
        ("push.negotiate".to_owned(), "false".to_owned()),
        ("protocol.git.allow".to_owned(), "never".to_owned()),
    ];
    let planted = |by: &dyn Fn(&Setting) -> bool| {
        settings
            .iter()
            .any(|setting| setting.of_the_repository() && by(setting))
    };
    if planted(&Setting::names_a_helper) {
        // An empty helper empties the list; the helpers of the person follow in their order.
        over.push(("credential.helper".to_owned(), String::new()));
        let own = settings
            .iter()
            .filter(|setting| !setting.of_the_repository() && setting.names_a_helper());
        over.extend(own.map(|setting| (setting.name.clone(), setting.value.clone())));
    }
    for (name, unset) in OF_THE_PERSON {
        if planted(&|setting| setting.name == name) {
            let own = settings
                .iter()
                .rfind(|setting| !setting.of_the_repository() && setting.name == name);
            let value = own.map_or(unset, |setting| setting.value.as_str());
            over.push((name.to_owned(), value.to_owned()));
        }
    }
    over
}

/// The settings of the repository, each name with a digest of its values, never a value: a
/// URL can hold a credential. The upstream of a branch and the note of git-lfs about a login
/// are left out; git writes them by itself and they name no program.
#[must_use]
pub fn of_the_repository(settings: &[Setting]) -> BTreeMap<String, String> {
    let mut digests: BTreeMap<String, Sha256> = BTreeMap::new();
    for setting in settings
        .iter()
        .filter(|setting| setting.of_the_repository())
    {
        let name = setting.name.as_str();
        if name.starts_with("branch.") || (name.starts_with("lfs.") && name.ends_with(".access")) {
            continue;
        }
        let digest = digests.entry(name.to_owned()).or_default();
        digest.update(setting.value.as_bytes());
        digest.update([0]);
    }
    digests
        .into_iter()
        .map(|(name, digest)| (name, crate::model::fetch::hex(&digest.finalize())))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const LISTED: &str = "system\0credential.helper\nosxkeychain\0\
        global\0gpg.program\n/usr/bin/gpg\0\
        global\0filter.lfs.clean\ngit-lfs clean\0\
        local\0core.bare\nfalse\0\
        local\0filter.evil.clean\nsh evil\0\
        local\0filter.evil.required\0\
        local\0credential.https://example.invalid.helper\n!evil\0\
        local\0gpg.program\n/tmp/evil\0\
        local\0core.askpass\n/tmp/ask\0\
        worktree\0commit.gpgsign\ntrue\0\
        local\0hook.evil.event\nreference-transaction\0\
        local\0hook.evil.command\ntouch RAN\0\
        global\0hook.My Lint.command\nlint\0\
        global\0commit.gpgsign\ntrue\0\
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
    fn only_the_filters_of_the_repository_are_turned_off() {
        let off = filters_off(&read(LISTED));
        let off: Vec<_> = off.iter().map(|(n, v)| (n.as_str(), v.as_str())).collect();
        assert_eq!(
            off,
            [("filter.evil.clean", ""), ("filter.evil.required", "false")]
        );
    }

    #[test]
    fn a_program_that_the_repository_names_is_replaced_by_the_one_of_the_person() {
        let over = for_the_person(&read(LISTED));
        let over: Vec<_> = over.iter().map(|(n, v)| (n.as_str(), v.as_str())).collect();
        assert_eq!(
            over,
            [
                ("push.pushOption", ""),
                ("push.negotiate", "false"),
                ("protocol.git.allow", "never"),
                ("credential.helper", ""),
                ("credential.helper", "osxkeychain"),
                ("core.askpass", ""),
                ("gpg.program", "/usr/bin/gpg"),
                ("commit.gpgsign", "true"),
            ]
        );
    }

    #[test]
    fn the_settings_of_the_repository_are_named_without_their_values() {
        let kept = of_the_repository(&read(LISTED));
        let names: Vec<_> = kept.keys().map(String::as_str).collect();
        assert_eq!(
            names,
            [
                "commit.gpgsign",
                "core.askpass",
                "core.bare",
                "credential.https://example.invalid.helper",
                "filter.evil.clean",
                "filter.evil.required",
                "gpg.program",
                "hook.evil.command",
                "hook.evil.event",
            ]
        );
        assert!(kept.values().all(|digest| digest.len() == 64));
        let other = LISTED.replace("/tmp/evil", "/tmp/else");
        assert_ne!(of_the_repository(&read(&other)), kept);
    }
}
