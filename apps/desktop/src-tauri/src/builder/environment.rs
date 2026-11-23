//! A build step gets an allow-list of the app's variables and nothing else: no signing key, token, URL with a secret or `DATABASE_URL` (BLD-042, BLD-044).

#[cfg(windows)]
mod windows;

use std::{
    ffi::{OsStr, OsString},
    path::{Path, PathBuf},
};

const PASSED: &[&str] = &[
    "HOME",
    "USER",
    "LOGNAME",
    "LANG",
    "LC_ALL",
    "LC_CTYPE",
    "TMPDIR",
    "CARGO_HOME",
    "RUSTUP_HOME",
    "RUSTUP_TOOLCHAIN",
    #[cfg(windows)]
    "USERPROFILE",
    #[cfg(windows)]
    "LOCALAPPDATA",
    #[cfg(windows)]
    "APPDATA",
    #[cfg(windows)]
    "SystemRoot",
    #[cfg(windows)]
    "WINDIR",
    #[cfg(windows)]
    "TEMP",
    #[cfg(windows)]
    "TMP",
    #[cfg(windows)]
    "INCLUDE",
    #[cfg(windows)]
    "LIB",
    #[cfg(windows)]
    "LIBPATH",
    #[cfg(windows)]
    "VCToolsInstallDir",
    #[cfg(windows)]
    "VCINSTALLDIR",
    #[cfg(windows)]
    "WindowsSdkDir",
    #[cfg(windows)]
    "WindowsSDKVersion",
];

/// Git runs with no user or system configuration: a credential helper or `url.insteadOf` there could redirect the clone.
const FIXED: &[(&str, &str)] = &[
    ("CI", "true"),
    ("NO_COLOR", "1"),
    ("CARGO_TERM_COLOR", "never"),
    ("GIT_TERMINAL_PROMPT", "0"),
    ("GIT_CONFIG_NOSYSTEM", "1"),
    (
        "GIT_CONFIG_GLOBAL",
        if cfg!(windows) { "NUL" } else { "/dev/null" },
    ),
    #[cfg(windows)]
    ("RUSTC_WRAPPER", ""),
    #[cfg(windows)]
    ("PATHEXT", ".COM;.EXE;.BAT;.CMD"),
    // Replace refs and grafts a build left cannot rewrite history (D-15); nothing can create a file under `/dev/null` (D-21).
    ("GIT_NO_REPLACE_OBJECTS", "1"),
    (
        "GIT_GRAFT_FILE",
        if cfg!(windows) {
            "NUL"
        } else {
            "/dev/null/anima-has-no-grafts"
        },
    ),
];

/// An app started from the Dock or a desktop menu often lacks the folders that rustup, bun, Homebrew and Go install into.
const EXTRA_DIRS: &[&str] = &[
    "/opt/homebrew/bin",
    "/usr/local/bin",
    "/usr/local/go/bin",
    "/usr/bin",
    "/bin",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Environment {
    passed: Vec<(OsString, OsString)>,
    dirs: Vec<PathBuf>,
}

impl Environment {
    pub fn from_parent(parent: impl IntoIterator<Item = (OsString, OsString)>) -> Self {
        let parent: Vec<(OsString, OsString)> = parent.into_iter().collect();
        let value = |name: &str| {
            parent
                .iter()
                .find(|(key, _)| {
                    if cfg!(windows) {
                        key.to_string_lossy().eq_ignore_ascii_case(name)
                    } else {
                        key == name
                    }
                })
                .map(|(_, value)| value.clone())
        };
        let passed = PASSED
            .iter()
            .filter_map(|name| value(name).map(|value| (OsString::from(name), value)))
            .collect();
        let dirs = search_dirs(
            value("PATH").as_deref(),
            value(if cfg!(windows) { "USERPROFILE" } else { "HOME" }).as_deref(),
            value("CARGO_HOME").as_deref(),
        );
        Self { passed, dirs }
    }

    /// Installed MSVC and SDK paths, discovered without executing a shell or build script.
    #[cfg(windows)]
    pub fn with_compiler(self) -> Self {
        windows::compiler(self)
    }

    /// A bundled native database runtime supplied by the shell, never by the web view.
    #[cfg(windows)]
    pub fn with_postgres(mut self, tree: PathBuf) -> Self {
        self.dirs.push(tree.join("bin"));
        self.passed
            .push(("ANIMA_POSTGRES_TREE".into(), tree.into_os_string()));
        self
    }

    pub fn dirs(&self) -> &[PathBuf] {
        &self.dirs
    }

    pub fn for_step(&self, step: &[(&'static str, String)]) -> Vec<(OsString, OsString)> {
        let mut vars = self.passed.clone();
        let path = std::env::join_paths(&self.dirs).unwrap_or_default();
        vars.push(("PATH".into(), path));
        vars.extend(
            FIXED
                .iter()
                .map(|(k, v)| (OsString::from(k), OsString::from(v))),
        );
        vars.extend(
            step.iter()
                .map(|(k, v)| (OsString::from(k), OsString::from(v))),
        );
        vars
    }

    pub fn find(&self, program: &str, exists: impl Fn(&Path) -> bool) -> Option<PathBuf> {
        self.dirs
            .iter()
            .map(|dir| {
                dir.join(
                    if cfg!(windows)
                        && !Path::new(program)
                            .extension()
                            .is_some_and(|ext| ext.eq_ignore_ascii_case("exe"))
                    {
                        format!("{program}.exe")
                    } else {
                        program.to_owned()
                    },
                )
            })
            .find(|path| exists(path))
    }
}

/// A relative entry is dropped: it would find programs in the clone, which the build writes.
fn search_dirs(path: Option<&OsStr>, home: Option<&OsStr>, cargo: Option<&OsStr>) -> Vec<PathBuf> {
    let home = home.map(PathBuf::from).filter(|home| home.is_absolute());
    let cargo_bin = cargo
        .map(|cargo| PathBuf::from(cargo).join("bin"))
        .or_else(|| home.as_ref().map(|home| home.join(".cargo/bin")));
    let bun_bin = home.as_ref().map(|home| home.join(".bun/bin"));
    let mut dirs: Vec<PathBuf> = Vec::new();
    let candidates = path
        .map(std::env::split_paths)
        .into_iter()
        .flatten()
        .chain(cargo_bin)
        .chain(bun_bin)
        .chain(EXTRA_DIRS.iter().map(PathBuf::from));
    for dir in candidates {
        if dir.is_absolute() && !dirs.contains(&dir) {
            dirs.push(dir);
        }
    }
    dirs
}
