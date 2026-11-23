//! The tools a builder needs before it claims a job (BLD-028). No updater key: the hub signs (R-117).
//! Pure: whether a path exists is a parameter, and nothing is run.

pub mod readings;

use std::path::{Path, PathBuf};

use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde::Serialize;

use super::{environment::Environment, job::Target};

/// One tool the Settings row names when it is missing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Tool {
    Git,
    Bun,
    /// Node runs the client DOM tests; Bun remains the package manager.
    Node,
    Rust,
    /// `cargo-nextest`, which gate 3 runs the Rust tests with (R-112).
    CargoNextest,
    /// `go`, which the six gates of `link/` run (D4 of the projects and builds proposal).
    Go,
    /// `MinGW` GCC, required by Go's race detector on Windows even for a pure Go link.
    GoRaceCompiler,
    Postgres,
    TauriPackages,
    /// On Linux, a program or the zlib header that `database-tree/build.sh` needs (S12 T3).
    TreeTools,
    /// The app's `plugins.updater.pubkey` is the placeholder, so what it builds could never update (R-108).
    UpdaterPublicKey,
    /// The login keychain has no valid identity named `bundle.macOS.signingIdentity` (BLD-083).
    SigningIdentity,
    /// No standard library for `aarch64-apple-darwin`; the universal app and `anima` need it (BLD-043, INS-002).
    #[serde(rename = "rust-target-aarch64-apple-darwin")]
    RustTargetAarch64Darwin,
    /// The same for `x86_64-apple-darwin`.
    #[serde(rename = "rust-target-x86_64-apple-darwin")]
    RustTargetX86_64Darwin,
}

const DARWIN_TARGETS: [(&str, Tool); 2] = [
    ("aarch64-apple-darwin", Tool::RustTargetAarch64Darwin),
    ("x86_64-apple-darwin", Tool::RustTargetX86_64Darwin),
];

/// None for Linux, which builds for the host.
pub fn rust_targets(
    target: Target,
    sysroot: Option<&Path>,
    exists: impl Fn(&Path) -> bool,
) -> Vec<Tool> {
    if target != Target::DarwinUniversal {
        return Vec::new();
    }
    DARWIN_TARGETS
        .iter()
        .filter(|(triple, _)| {
            !sysroot.is_some_and(|root| exists(&root.join("lib/rustlib").join(triple).join("lib")))
        })
        .map(|(_, tool)| *tool)
        .collect()
}

/// The length of a minisign public key: `Ed`, the 8-byte key ID and the 32-byte key.
const PUBLIC_KEY_BYTES: usize = 42;

/// `encoded` is the base64 of the public key file (`plugins.updater.pubkey`).
pub fn key_line(encoded: &str) -> Option<String> {
    let text = String::from_utf8(STANDARD.decode(encoded).ok()?).ok()?;
    text.lines()
        .map(str::trim)
        .rfind(|line| !line.is_empty())
        .map(str::to_owned)
}

/// The placeholder has no key line (R-108).
pub fn app_public_key(line: Option<&str>) -> Option<Tool> {
    let key = line
        .and_then(|line| STANDARD.decode(line).ok())
        .is_some_and(|bytes| bytes.len() == PUBLIC_KEY_BYTES && bytes.starts_with(b"Ed"));
    (!key).then_some(Tool::UpdaterPublicKey)
}

/// `security find-identity -v` lists valid identities only, each name in double quotes (BLD-083).
pub fn signing_identity(identity: Option<&str>, keychain: &[String]) -> Option<Tool> {
    let found = identity
        .filter(|name| !name.is_empty())
        .map(|name| format!("\"{name}\""))
        .is_some_and(|quoted| keychain.iter().any(|line| line.contains(&quoted)));
    (!found).then_some(Tool::SigningIdentity)
}

/// The PostgreSQL major version the gates start (DB-001, R-009).
const POSTGRES_MAJOR: u8 = 17;

/// The folders `quality/src/gates/postgres.ts` searches before `PATH`.
fn postgres_dirs(path_dirs: &[PathBuf]) -> Vec<PathBuf> {
    let major = POSTGRES_MAJOR;
    let mut dirs = vec![
        PathBuf::from(format!("/opt/homebrew/opt/postgresql@{major}/bin")),
        PathBuf::from(format!("/usr/local/opt/postgresql@{major}/bin")),
        PathBuf::from(format!("/usr/lib/postgresql/{major}/bin")),
    ];
    dirs.extend_from_slice(path_dirs);
    dirs
}

const PKG_CONFIG_DIRS: &[&str] = &[
    "/usr/lib/x86_64-linux-gnu/pkgconfig",
    "/usr/lib64/pkgconfig",
    "/usr/lib/pkgconfig",
    "/usr/share/pkgconfig",
    "/usr/local/lib/pkgconfig",
];

/// The development packages the Tauri shell links on Linux (quality/README.md).
const LINUX_PACKAGES: &[&str] = &["webkit2gtk-4.1", "libsoup-3.0", "librsvg-2.0", "dbus-1"];

const MAC_DEVELOPER_DIRS: &[&str] = &[
    "/Library/Developer/CommandLineTools/usr/bin",
    "/Applications/Xcode.app/Contents/Developer/usr/bin",
];

fn tauri_packages(target: Target, env: &Environment, exists: &impl Fn(&Path) -> bool) -> bool {
    match target {
        Target::LinuxX86_64 => {
            env.find("pkg-config", exists).is_some()
                && env.find("cc", exists).is_some()
                && LINUX_PACKAGES.iter().all(|package| {
                    PKG_CONFIG_DIRS
                        .iter()
                        .any(|dir| exists(&Path::new(dir).join(format!("{package}.pc"))))
                })
        }
        Target::WindowsX86_64 => {
            env.find("link", exists).is_some() && env.find("rc", exists).is_some()
        }
        Target::DarwinUniversal => MAC_DEVELOPER_DIRS
            .iter()
            .any(|dir| exists(&Path::new(dir).join("clang"))),
    }
}

/// The recipe runs with `PATH=/usr/bin:/bin` only, so each program must be in one of those two folders.
const TREE_PROGRAMS: &[&str] = &["make", "bison", "flex", "patchelf", "readelf", "curl"];
const TREE_FOLDERS: &[&str] = &["/usr/bin", "/bin"];
const ZLIB_HEADER: &str = "/usr/include/zlib.h";

/// On macOS the Xcode tools have it, and [`tauri_packages`] asks for them already.
fn tree_tools(target: Target, exists: &impl Fn(&Path) -> bool) -> bool {
    match target {
        Target::LinuxX86_64 => {
            exists(Path::new(ZLIB_HEADER))
                && TREE_PROGRAMS.iter().all(|program| {
                    TREE_FOLDERS
                        .iter()
                        .any(|folder| exists(&Path::new(folder).join(program)))
                })
        }
        Target::DarwinUniversal | Target::WindowsX86_64 => true,
    }
}

/// In the order the Settings row lists them; `psql` is not one, as the gates never run it.
pub fn missing(target: Target, env: &Environment, exists: impl Fn(&Path) -> bool) -> Vec<Tool> {
    let has = |program: &str| env.find(program, &exists).is_some();
    let postgres = postgres_dirs(env.dirs()).iter().any(|dir| {
        let suffix = if cfg!(windows) { ".exe" } else { "" };
        exists(&dir.join(format!("initdb{suffix}"))) && exists(&dir.join(format!("pg_ctl{suffix}")))
    });
    [
        (Tool::Git, has("git")),
        (Tool::Bun, has("bun")),
        (Tool::Node, target != Target::WindowsX86_64 || has("node")),
        (Tool::Rust, has("cargo") && has("rustc")),
        (Tool::CargoNextest, has("cargo-nextest")),
        (Tool::Go, has("go")),
        (
            Tool::GoRaceCompiler,
            target != Target::WindowsX86_64 || has("gcc"),
        ),
        (Tool::Postgres, postgres),
        (Tool::TauriPackages, tauri_packages(target, env, &exists)),
        (Tool::TreeTools, tree_tools(target, &exists)),
    ]
    .into_iter()
    .filter_map(|(tool, present)| (!present).then_some(tool))
    .collect()
}
