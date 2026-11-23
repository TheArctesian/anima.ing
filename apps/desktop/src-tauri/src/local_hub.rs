//! The hub on this computer (one-click plan S4); no rule of the hub is here (REPO-003).
//! Only the main web view may ask (WEB-007), and no path, program or address comes from it.
//! The account commands pass secrets on standard input and never log them (LOG-007).

pub mod account;
mod account_form;
mod carried;
mod error;
mod key_copy;
mod recovery;
pub mod update;

#[cfg(all(test, unix))]
mod tests;
#[cfg(all(test, windows))]
mod windows_tests;

use std::{
    ffi::OsString,
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use tauri::{Runtime, State, Webview};
use tauri_plugin_log::log;

use crate::carried::run::{self as program, Invocation};
pub use account::Replacements;
use carried::Carried;
pub use error::LocalHubError;

/// Above the worst case of `install` (stop, wait, three Linux starts, 120 s each, and the copy); a kill would leave the hub stopped.
const INSTALL_LIMIT: Duration = Duration::from_mins(12);
/// `remove`: the stop command, the wait and on Linux one more command, 120 s each.
const REMOVE_LIMIT: Duration = Duration::from_mins(7);
const STATUS_LIMIT: Duration = Duration::from_secs(10);
const START_STOP_LIMIT: Duration = Duration::from_mins(3);

/// Only these variables reach the program; `systemctl --user` needs the session bus.
#[cfg(unix)]
const PASSED_ON: [&str; 4] = [
    "HOME",
    "XDG_DATA_HOME",
    "XDG_RUNTIME_DIR",
    "DBUS_SESSION_BUS_ADDRESS",
];

#[cfg(windows)]
const PASSED_ON: [&str; 6] = [
    "USERPROFILE",
    "LOCALAPPDATA",
    "SystemRoot",
    "WINDIR",
    "TEMP",
    "TMP",
];

/// Only one change of the hub on this computer (install, remove, start, stop, account) runs at a time.
#[derive(Debug, Default)]
pub struct Changing(Arc<AtomicBool>);

struct Claim(Arc<AtomicBool>);

impl Changing {
    fn claim(&self) -> Result<Claim, LocalHubError> {
        if self.0.swap(true, Ordering::SeqCst) {
            return Err(LocalHubError::Busy);
        }
        Ok(Claim(Arc::clone(&self.0)))
    }
}

impl Drop for Claim {
    fn drop(&mut self) {
        self.0.store(false, Ordering::SeqCst);
    }
}

fn environment(
    read: impl Fn(&str) -> Option<OsString>,
) -> Result<Vec<(&'static str, OsString)>, LocalHubError> {
    let home_is_absolute = read(if cfg!(windows) { "USERPROFILE" } else { "HOME" })
        .is_some_and(|home| Path::new(&home).is_absolute());
    if !home_is_absolute {
        return Err(LocalHubError::NoHome);
    }
    Ok(PASSED_ON
        .into_iter()
        .filter_map(|name| Some((name, read(name)?)))
        .collect())
}

fn carried() -> Result<Carried, LocalHubError> {
    if !cfg!(any(
        target_os = "macos",
        target_os = "linux",
        target_os = "windows"
    )) {
        return Err(LocalHubError::Unavailable);
    }
    let executable = std::env::current_exe().map_err(|_| LocalHubError::NotCarried)?;
    carried::beside(
        &executable,
        cfg!(target_os = "macos"),
        !cfg!(debug_assertions),
        &crate::carried::DevFolders::of_this_run(),
    )
}

fn ask(carried: &Carried, args: &[&OsString], limit: Duration) -> Result<Vec<u8>, LocalHubError> {
    let env = environment(|name| std::env::var_os(name))?;
    let invocation = Invocation {
        program: &carried.program,
        args,
        env: &env,
        limit,
    };
    program::run(&invocation).map_err(|error| {
        log::warn!("anima-local-hub: {error}");
        LocalHubError::from(&error)
    })
}

fn install_args(tree: &Path) -> [OsString; 3] {
    ["install".into(), "--tree".into(), tree.into()]
}

fn parse_status(printed: &[u8]) -> Result<serde_json::Value, LocalHubError> {
    let status: serde_json::Value =
        serde_json::from_slice(printed).map_err(|_| LocalHubError::Failed)?;
    let is_status = status
        .get("installed")
        .is_some_and(serde_json::Value::is_boolean)
        && status
            .get("running")
            .is_some_and(serde_json::Value::is_boolean);
    if !is_status {
        return Err(LocalHubError::Failed);
    }
    Ok(status)
}

/// The second wall after the capability (WEB-007).
fn from_main(label: &str) -> Result<(), LocalHubError> {
    crate::web_view::caller(label).map_err(|_| LocalHubError::Caller)
}

/// The claim goes with the work, so it is freed when the work ends.
async fn change(
    changing: &Changing,
    work: impl FnOnce(&Carried) -> Result<Vec<u8>, LocalHubError> + Send + 'static,
) -> Result<(), LocalHubError> {
    let claim = changing.claim()?;
    let work = move || {
        let _claim = claim;
        work(&carried()?).map(drop)
    };
    tauri::async_runtime::spawn_blocking(work)
        .await
        .unwrap_or(Err(LocalHubError::Failed))
}

#[tauri::command]
pub async fn local_hub_carried<R: Runtime>(webview: Webview<R>) -> Result<bool, LocalHubError> {
    from_main(webview.label())?;
    Ok(carried().is_ok())
}

#[tauri::command]
pub async fn local_hub_install<R: Runtime>(
    webview: Webview<R>,
    changing: State<'_, Changing>,
) -> Result<(), LocalHubError> {
    from_main(webview.label())?;
    let result = change(&changing, |carried| {
        let args = install_args(&carried.tree);
        ask(carried, &args.each_ref(), INSTALL_LIMIT)
    })
    .await;
    match &result {
        Ok(()) => log::info!("the hub was installed on this computer"),
        Err(error) => log::warn!("the hub was not installed: {}", error.code()),
    }
    result
}

/// Stops the hub on this computer and removes its unit and programs. Its database and key stay.
#[tauri::command]
pub async fn local_hub_remove<R: Runtime>(
    webview: Webview<R>,
    changing: State<'_, Changing>,
) -> Result<(), LocalHubError> {
    from_main(webview.label())?;
    let result = change(&changing, |carried| {
        ask(carried, &[&OsString::from("remove")], REMOVE_LIMIT)
    })
    .await;
    match &result {
        Ok(()) => log::info!("the hub was removed from this computer"),
        Err(error) => log::warn!("the hub was not removed: {}", error.code()),
    }
    result
}

async fn start_or_stop(changing: &Changing, verb: &'static str) -> Result<(), LocalHubError> {
    let result = change(changing, move |carried| {
        ask(carried, &[&OsString::from(verb)], START_STOP_LIMIT)
    })
    .await;
    match &result {
        Ok(()) => log::info!("the hub on this computer was told to {verb}"),
        Err(error) => log::warn!("the hub did not {verb}: {}", error.code()),
    }
    result
}

#[tauri::command]
pub async fn local_hub_start<R: Runtime>(
    webview: Webview<R>,
    changing: State<'_, Changing>,
) -> Result<(), LocalHubError> {
    from_main(webview.label())?;
    start_or_stop(&changing, "start").await
}

/// Stops the hub on this computer until `start` or the next sign-in to the computer.
#[tauri::command]
pub async fn local_hub_stop<R: Runtime>(
    webview: Webview<R>,
    changing: State<'_, Changing>,
) -> Result<(), LocalHubError> {
    from_main(webview.label())?;
    start_or_stop(&changing, "stop").await
}

#[tauri::command]
pub async fn local_hub_status<R: Runtime>(
    webview: Webview<R>,
) -> Result<serde_json::Value, LocalHubError> {
    from_main(webview.label())?;
    let work = || -> Result<serde_json::Value, LocalHubError> {
        let printed = ask(&carried()?, &[&OsString::from("status")], STATUS_LIMIT)?;
        parse_status(&printed)
    };
    tauri::async_runtime::spawn_blocking(work)
        .await
        .unwrap_or(Err(LocalHubError::Failed))
}
