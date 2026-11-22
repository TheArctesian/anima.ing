//! The rules of `.gitignore` files that the `@` list keeps out (TH108), as `rg --files` does:
//! blank lines and `#` comments, `!` that takes a path back, a `/` at the end for a folder only,
//! a `/` inside or at the start that ties the rule to the folder of its file, and `*`, `?` and
//! `**`. The last rule that matches decides. A folder that is left out is not entered, so a rule
//! below it cannot take a path back, as in git. Character classes (`[abc]`) are read as text.

/// The rules of the `.gitignore` files on the way to a folder, the outer ones first.
#[derive(Clone, Debug, Default)]
pub struct Ignored {
    rules: Vec<Rule>,
}

#[derive(Clone, Debug)]
struct Rule {
    /// The folder of the file of the rule, from the working folder (empty at the top), with a
    /// `/` at its end when it is not empty.
    base: String,
    glob: String,
    /// The rule names whole paths from `base`, not only names.
    anchored: bool,
    folder_only: bool,
    takes_back: bool,
}

impl Ignored {
    /// These rules and the rules of the text of a `.gitignore` in the folder `base` (from the
    /// working folder, empty at the top).
    #[must_use]
    pub fn with(&self, base: &str, text: &str) -> Self {
        let base = if base.is_empty() {
            String::new()
        } else {
            format!("{base}/")
        };
        let mut rules = self.rules.clone();
        rules.extend(text.lines().filter_map(|line| Rule::read(line, &base)));
        Self { rules }
    }

    /// Whether the rules leave out `path` (from the working folder).
    #[must_use]
    pub fn leaves_out(&self, path: &str, folder: bool) -> bool {
        self.rules
            .iter()
            .rev()
            .find(|rule| rule.matches(path, folder))
            .is_some_and(|rule| !rule.takes_back)
    }
}

impl Rule {
    fn read(line: &str, base: &str) -> Option<Self> {
        let line = line.trim_end();
        if line.is_empty() || line.starts_with('#') {
            return None;
        }
        let (takes_back, line) = line
            .strip_prefix('!')
            .map_or((false, line), |rest| (true, rest));
        let line = line.strip_prefix('\\').unwrap_or(line);
        let (folder_only, line) = line
            .strip_suffix('/')
            .map_or((false, line), |rest| (true, rest));
        let anchored = line.contains('/');
        let glob = line.strip_prefix('/').unwrap_or(line);
        (!glob.is_empty()).then(|| Self {
            base: base.to_owned(),
            glob: glob.to_owned(),
            anchored,
            folder_only,
            takes_back,
        })
    }

    fn matches(&self, path: &str, folder: bool) -> bool {
        if self.folder_only && !folder {
            return false;
        }
        let Some(below) = path.strip_prefix(&self.base) else {
            return false;
        };
        if self.anchored {
            glob(self.glob.as_bytes(), below.as_bytes())
        } else {
            let name = below.rsplit('/').next().unwrap_or(below);
            glob(self.glob.as_bytes(), name.as_bytes())
        }
    }
}

/// Whether `text` matches `pattern`: `*` and `?` stay in one part of a path, `**` crosses parts.
fn glob(pattern: &[u8], text: &[u8]) -> bool {
    match pattern {
        [] => text.is_empty(),
        [b'*', b'*', b'/', rest @ ..] => {
            glob(rest, text)
                || text
                    .iter()
                    .enumerate()
                    .any(|(at, byte)| *byte == b'/' && glob(rest, &text[at + 1..]))
        }
        [b'*', b'*', rest @ ..] => (0..=text.len()).any(|at| glob(rest, &text[at..])),
        [b'*', rest @ ..] => (0..=text.len())
            .take_while(|at| *at == 0 || text[at - 1] != b'/')
            .any(|at| glob(rest, &text[at..])),
        [b'?', rest @ ..] => text
            .split_first()
            .is_some_and(|(byte, more)| *byte != b'/' && glob(rest, more)),
        [want, rest @ ..] => text
            .split_first()
            .is_some_and(|(byte, more)| byte == want && glob(rest, more)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn th108_the_rules_of_gitignore_files_leave_paths_out() {
        let top = Ignored::default().with(
            "",
            "# build\n\n*.log\n/out/\nbuild/\n!keep.log\ndocs/**/draft-*.md\n\\#notes\n",
        );
        let nested = top.with("app", "*.tmp\n/local\n");
        for (path, folder) in [
            ("a.log", false),
            ("src/deep/b.log", false),
            ("out", true),
            ("src/build", true),
            ("docs/x/y/draft-1.md", false),
            ("docs/draft-2.md", false),
            ("#notes", false),
        ] {
            assert!(top.leaves_out(path, folder), "{path}");
        }
        for (path, folder) in [
            ("keep.log", false),
            ("src/out", true),
            ("build", false),
            ("docs/final.md", false),
            ("logs", true),
        ] {
            assert!(!top.leaves_out(path, folder), "{path}");
        }
        assert!(nested.leaves_out("app/x/y.tmp", false));
        assert!(nested.leaves_out("app/local", false));
        assert!(!nested.leaves_out("app/x/local", false));
        assert!(!nested.leaves_out("other/y.tmp", false));
        assert!(nested.leaves_out("app/z.log", false));
    }
}
