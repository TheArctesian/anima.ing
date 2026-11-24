//! Installs the carried metrics daemon for this user, never as root (MET-019, D-10 d, HMD-052,
//! HMD-055, HMD-056). Only the main web view may ask (WEB-007). The enrolment code reaches the
//! daemon on standard input only; no log line, error or argument holds it (LOG-007).
//! Only the shell's own copy of the code is wiped (`Zeroizing`); the IPC message and the web view keep theirs.

#[cfg(unix)]
pub(crate) mod at_start;
mod error;
mod installing;
#[cfg(unix)]
pub(crate) mod keychain;
#[cfg_attr(windows, path = "daemon/placement/windows.rs")]
pub(crate) mod placement;
mod removal;
mod request;
mod run;
#[cfg(unix)]
mod signature;
#[cfg(unix)]
pub(crate) mod trust;
#[cfg(unix)]
mod update;
#[cfg(unix)]
mod version;

#[cfg(all(test, unix))]
mod tests;

use std::{
    ffi::OsString,
    path::{Path, PathBuf},
    sync::atomic::AtomicBool,
    time::Duration,
};

use tauri::{Runtime, State, Webview};
use tauri_plugin_log::log;
use zeroize::Zeroizing;

#[cfg(unix)]
pub use at_start::{Notices, update_at_start};
pub use error::DaemonError;
pub use installing::Installing;
use request::{EnrolmentCode, HubOrigin};
use run::{INSTALL_ARGS, Run};
#[cfg(unix)]
use std::fs;
#[cfg(unix)]
use trust::User;

/// The hub answers an enrolment within its own 10 s timeout; a macOS Keychain write adds a little.
const ENROL_DEADLINE: Duration = Duration::from_secs(60);
/// `uninstall` runs at most two steps of 30 s each and deletes a Keychain item; the rest is margin.
const UNINSTALL_DEADLINE: Duration = Duration::from_secs(120);
/// `install` runs up to five steps of 30 s each (R-126), one retried; its Keychain read stops
/// after 5 s (R-322), so the deadline is for slow service managers.
const INSTALL_DEADLINE: Duration = Duration::from_secs(240);

#[cfg(unix)]
const PASSWD: &str = "/etc/passwd";
#[cfg(unix)]
const GROUP: &str = "/etc/group";

struct Here {
    source: PathBuf,
    home: PathBuf,
    #[cfg(unix)]
    user: User,
    env: Vec<(&'static str, OsString)>,
}

struct Limits<'a> {
    enrol: Duration,
    install: Duration,
    stop: &'a AtomicBool,
}

/// Each run starts the path that was checked.
fn install_here(
    here: &Here,
    code: &EnrolmentCode,
    hub: &HubOrigin,
    limits: &Limits<'_>,
) -> Result<PathBuf, DaemonError> {
    let destination = placement::destination(&here.home);
    #[cfg(unix)]
    let placed = placement::place(&here.source, &destination, here.user);
    #[cfg(windows)]
    let placed = placement::place(&here.source, &destination);
    let installed = placed.map_err(DaemonError::of_placement)?;
    let input = code.input_line();
    let enrol = Run {
        program: &installed,
        args: &run::enrol_args(hub.as_str()),
        input: Some(&input),
        env: &here.env,
        deadline: limits.enrol,
        stop: limits.stop,
    };
    run::run(&enrol).map_err(|error| DaemonError::of_run(error, DaemonError::Enrol))?;
    let install = Run {
        program: &installed,
        args: &INSTALL_ARGS,
        input: None,
        env: &here.env,
        deadline: limits.install,
        stop: limits.stop,
    };
    run::run(&install).map_err(|error| DaemonError::of_run(error, DaemonError::Install))?;
    Ok(installed)
}

fn here(source: PathBuf) -> Result<Here, DaemonError> {
    let home = home_folder()?;
    #[cfg(unix)]
    let user = this_user();
    let env = run::daemon_env(&home, |name| std::env::var_os(name));
    Ok(Here {
        source,
        home,
        #[cfg(unix)]
        user,
        env,
    })
}

fn home_folder() -> Result<PathBuf, DaemonError> {
    std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" })
        .map(PathBuf::from)
        .filter(|home| home.is_absolute())
        .ok_or(DaemonError::NoHome)
}

/// An account file that cannot be read means no private group, so a folder its group may write
/// is refused.
#[cfg(unix)]
pub(crate) fn this_user() -> User {
    let uid = nix::unistd::getuid().as_raw();
    let passwd = fs::read_to_string(PASSWD).ok();
    let group = fs::read_to_string(GROUP).ok();
    User {
        uid,
        private_group: trust::private_group(uid, passwd.as_deref(), group.as_deref()),
    }
}

/// A debug build's target folder may hold a daemon built from another commit, which is never
/// offered.
fn bundled(executable: &Path, release: bool) -> Result<PathBuf, DaemonError> {
    if !release {
        return Err(DaemonError::NotBundled);
    }
    placement::bundled_beside(executable)
        .filter(|daemon| daemon.is_file())
        .ok_or(DaemonError::NotBundled)
}

fn carried() -> Result<PathBuf, DaemonError> {
    if !cfg!(any(target_os = "macos", target_os = "linux", windows)) {
        return Err(DaemonError::Unavailable);
    }
    let executable = std::env::current_exe().map_err(|_| DaemonError::NotBundled)?;
    bundled(&executable, !cfg!(debug_assertions))
}

/// Refuses every caller but the main web view, a second wall behind the capability (WEB-007).
fn from_main(label: &str) -> Result<(), DaemonError> {
    crate::web_view::caller(label).map_err(|_| DaemonError::Caller)
}

/// The claim moves into the work, so it is freed when the work ends even if no `await` resumes.
async fn install<R: Runtime>(
    webview: &Webview<R>,
    installing: &Installing,
    code: Zeroizing<String>,
    origin: &str,
) -> Result<(), DaemonError> {
    from_main(webview.label())?;
    let code = EnrolmentCode::parse(&code).ok_or(DaemonError::Code)?;
    let hub = hub_origin(origin).ok_or(DaemonError::Origin)?;
    let claim = installing.claim()?;
    let work = move || {
        let limits = Limits {
            enrol: ENROL_DEADLINE,
            install: INSTALL_DEADLINE,
            stop: &claim.stop,
        };
        install_here(&here(carried()?)?, &code, &hub, &limits).map(drop)
    };
    tauri::async_runtime::spawn_blocking(work)
        .await
        .unwrap_or(Err(DaemonError::Failed))
}

async fn remove<R: Runtime>(
    webview: &Webview<R>,
    installing: &Installing,
) -> Result<bool, DaemonError> {
    from_main(webview.label())?;
    let claim = installing.claim()?;
    let work = move || {
        let home = home_folder()?;
        let carried = carried().ok();
        let env = run::daemon_env(&home, |name| std::env::var_os(name));
        removal::remove_here(&removal::Leave {
            copy: &placement::destination(&home),
            carried: carried.as_deref(),
            env: &env,
            deadline: UNINSTALL_DEADLINE,
            stop: &claim.stop,
        })
    };
    tauri::async_runtime::spawn_blocking(work)
        .await
        .unwrap_or(Err(DaemonError::Failed))
}

/// A hub behind the link at `http://127.0.0.1:<port>` becomes its daemon socket; the web view
/// never names a path (HMD-054).
fn hub_origin(origin: &str) -> Option<HubOrigin> {
    HubOrigin::parse(origin).or_else(|| {
        let socket = crate::link::daemon_socket(request::link_port(origin)?)?;
        HubOrigin::link(&socket)
    })
}

#[tauri::command]
pub async fn daemon_bundled<R: Runtime>(webview: Webview<R>) -> Result<bool, DaemonError> {
    from_main(webview.label())?;
    Ok(carried().is_ok())
}

#[tauri::command]
pub async fn daemon_install<R: Runtime>(
    webview: Webview<R>,
    installing: State<'_, Installing>,
    code: String,
    origin: String,
) -> Result<(), DaemonError> {
    let result = install(&webview, &installing, Zeroizing::new(code), &origin).await;
    match &result {
        Ok(()) => log::info!("the daemon was installed on this computer"),
        Err(error) => log::warn!("the daemon was not installed: {}", error.code()),
    }
    result
}

/// Takes the daemon's unit, token and copy off this computer (HMD-056); true when a copy was here.
#[tauri::command]
pub async fn daemon_remove<R: Runtime>(
    webview: Webview<R>,
    installing: State<'_, Installing>,
) -> Result<bool, DaemonError> {
    let result = remove(&webview, &installing).await;
    match &result {
        Ok(true) => log::info!("the daemon was removed from this computer"),
        Ok(false) => log::info!("no copy of the daemon was on this computer to remove"),
        Err(error) => log::warn!("the daemon was not removed: {}", error.code()),
    }
    result
}
