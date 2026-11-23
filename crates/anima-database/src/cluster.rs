//! The PostgreSQL data folder of one bundled database.
//!
//! The built-in `C.UTF-8` locale provider keeps indexes safe from OS updates; checksums stay on
//! because an upgrade needs both sides equal.
//! The superuser signs in only by `peer`, every other login by `scram-sha-256`, and the server
//! has no network listener (DB-001).

use std::fs;
use std::io::{self, Write};
use std::path::Path;

#[cfg(unix)]
use nix::unistd::{Uid, User};

use crate::{Layout, Lock, Program, Tree, owner_only};

#[cfg(all(test, unix))]
mod tests;

/// The superuser of a bundled cluster; only this crate signs in with it, never the hub.
pub const SUPERUSER: &str = "anima_super";

#[cfg(unix)]
const USER_MAP: &str = "anima";

#[cfg(unix)]
const ACCESS_RULES: &str = "\
# Written by anima-database; a change here is lost when the cluster is made again.
# The superuser: only the operating-system user who runs the database, no password.
local all anima_super peer map=anima
# Every other login: its password, never in clear text.
local all all scram-sha-256
";

#[cfg(windows)]
const ACCESS_RULES: &str = "hostssl all all 127.0.0.1/32 scram-sha-256\nhost all all 0.0.0.0/0 reject\nhost all all ::0/0 reject\n";

#[derive(Debug, thiserror::Error)]
pub enum ClusterError {
    #[error("could not {step}")]
    Io {
        step: &'static str,
        #[source]
        source: io::Error,
    },
    #[error("initdb failed: {reason}")]
    Initdb { reason: String },
    #[error("the cluster's PG_VERSION file holds no version number")]
    VersionFile,
    #[error("a cluster folder is there already; a new cluster is never made over it")]
    AlreadyThere,
    #[error("the operating system has no name for the user who runs the database")]
    UserName,
    #[error(
        "the name of the user who runs the database starts with `/` or has a control \
         character; PostgreSQL would read it as a pattern or as two lines"
    )]
    UserNameForm,
    #[error("pg_controldata did not say how the cluster was stopped")]
    ControlText,
}

fn io_error(step: &'static str) -> impl FnOnce(io::Error) -> ClusterError {
    move |source| ClusterError::Io { step, source }
}

/// How a cluster's major version stands to the tree that would run it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fit {
    Same,
    /// The cluster needs an upgrade before this tree can run it.
    ClusterOlder {
        cluster: u32,
        tree: u32,
    },
    /// A newer application made or upgraded the cluster; this tree does not touch it.
    ClusterNewer {
        cluster: u32,
        tree: u32,
    },
}

#[must_use]
pub fn fit(cluster: u32, tree: u32) -> Fit {
    match cluster.cmp(&tree) {
        std::cmp::Ordering::Equal => Fit::Same,
        std::cmp::Ordering::Less => Fit::ClusterOlder { cluster, tree },
        std::cmp::Ordering::Greater => Fit::ClusterNewer { cluster, tree },
    }
}

/// The major version in the cluster's `PG_VERSION`, or `None` when no cluster is there yet.
///
/// # Errors
/// When the file cannot be read or holds no number.
pub fn major_version(layout: &Layout) -> Result<Option<u32>, ClusterError> {
    match fs::read_to_string(layout.cluster().join("PG_VERSION")) {
        Ok(text) => text
            .trim()
            .parse()
            .map(Some)
            .map_err(|_| ClusterError::VersionFile),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(source) => Err(io_error("read the cluster's PG_VERSION")(source)),
    }
}

/// Makes the cluster with `initdb`; the caller holds the [`Lock`].
///
/// The cluster is made beside its place and moved in last, so a crash leaves none or a whole one.
///
/// # Errors
/// When a cluster folder is there already, a file cannot be made, or `initdb` fails.
pub fn create(_lock: &Lock, tree: &Tree, layout: &Layout) -> Result<(), ClusterError> {
    if layout.cluster().exists() {
        return Err(ClusterError::AlreadyThere);
    }
    owner_only::folder(layout.root()).map_err(io_error("make the database root"))?;
    let being_made = layout.cluster_being_made();
    if let Err(error) = fs::remove_dir_all(&being_made)
        && error.kind() != io::ErrorKind::NotFound
    {
        return Err(io_error("remove a half-made cluster")(error));
    }
    #[cfg(windows)]
    crate::windows::Endpoint::create(layout).map_err(io_error("prepare the database identity"))?;
    run_initdb(tree, &being_made)?;
    replace(&being_made.join("pg_hba.conf"), ACCESS_RULES)
        .map_err(io_error("write the cluster's access rules"))?;
    #[cfg(unix)]
    replace(&being_made.join("pg_ident.conf"), &user_map(&this_user()?))
        .map_err(io_error("write the cluster's user map"))?;
    fs::rename(&being_made, layout.cluster()).map_err(io_error("move the new cluster into place"))
}

/// Lets only the operating-system user who runs this program be the superuser; run before each
/// start, because the user's name can change.
///
/// # Errors
/// When the user has no name or the map cannot be written.
#[cfg(unix)]
pub fn admit_this_user(_lock: &Lock, layout: &Layout) -> Result<(), ClusterError> {
    replace(
        &layout.cluster().join("pg_ident.conf"),
        &user_map(&this_user()?),
    )
    .map_err(io_error("write the cluster's user map"))
}

#[cfg(windows)]
pub fn admit_this_user(_lock: &Lock, layout: &Layout) -> Result<(), ClusterError> {
    crate::windows::Endpoint::load(layout)
        .map(drop)
        .map_err(io_error("check the database identity"))
}

#[cfg(unix)]
fn this_user() -> Result<String, ClusterError> {
    let name = User::from_uid(Uid::effective())
        .ok()
        .flatten()
        .map(|user| user.name)
        .ok_or(ClusterError::UserName)?;
    if is_plain_user_name(&name) {
        Ok(name)
    } else {
        Err(ClusterError::UserNameForm)
    }
}

/// In `pg_ident.conf` a leading `/` makes a value a regular expression and a line break starts a mapping.
#[cfg(unix)]
fn is_plain_user_name(name: &str) -> bool {
    !name.is_empty() && !name.starts_with('/') && !name.chars().any(char::is_control)
}

#[cfg(unix)]
fn user_map(user: &str) -> String {
    format!(
        "# Written by anima-database at each start.\n{USER_MAP} \"{}\" {SUPERUSER}\n",
        user.replace('"', "\"\"")
    )
}

/// Writes `text` beside `file` and moves it in, so the server never reads half a file.
fn replace(file: &Path, text: &str) -> io::Result<()> {
    let mut beside = file.to_path_buf().into_os_string();
    beside.push(".new");
    let mut new = owner_only::new_file(beside.as_ref())?;
    new.write_all(text.as_bytes())?;
    new.sync_all()?;
    fs::rename(&beside, file)
}

/// Whether the cluster's last server wrote its shutdown checkpoint. `false` after a kill or
/// a power cut: the next start recovers from the write-ahead log.
///
/// # Errors
/// When `pg_controldata` cannot run or prints no state.
pub fn was_stopped_cleanly(tree: &Tree, layout: &Layout) -> Result<bool, ClusterError> {
    let output = tree
        .command(Program::PgControldata)
        .arg(layout.cluster())
        .output()
        .map_err(io_error("run pg_controldata"))?;
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .find_map(|line| line.strip_prefix("Database cluster state:"))
        .map(|state| state.trim() == "shut down")
        .ok_or(ClusterError::ControlText)
}

fn run_initdb(tree: &Tree, cluster: &Path) -> Result<(), ClusterError> {
    let mut command = tree.command(Program::Initdb);
    #[cfg(unix)]
    command.args(["--auth-local", "peer", "--auth-host", "reject"]);
    #[cfg(windows)]
    command
        .args([
            "--auth-local",
            "scram-sha-256",
            "--auth-host",
            "scram-sha-256",
        ])
        .arg("--pwfile")
        .arg(
            cluster
                .parent()
                .ok_or(ClusterError::UserName)?
                .join("connection/superuser.password"),
        );
    let output = command
        .arg("--pgdata")
        .arg(cluster)
        .args(["--username", SUPERUSER])
        .args(["--encoding", "UTF8", "--locale", "C"])
        .args([
            "--locale-provider",
            "builtin",
            "--builtin-locale",
            "C.UTF-8",
        ])
        .arg("--data-checksums")
        .output()
        .map_err(io_error("run initdb"))?;
    if output.status.success() {
        return Ok(());
    }
    Err(ClusterError::Initdb {
        reason: initdb_reason(&String::from_utf8_lossy(&output.stderr)),
    })
}

fn initdb_reason(errors: &str) -> String {
    let own = errors
        .lines()
        .find_map(|line| line.strip_prefix("initdb: error: "));
    let of_server = errors
        .lines()
        .find(|line| line.contains("FATAL:"))
        .map(str::trim);
    own.or(of_server)
        .unwrap_or("it printed no cause")
        .to_owned()
}
