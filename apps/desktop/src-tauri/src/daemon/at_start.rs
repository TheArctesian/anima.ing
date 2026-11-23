//! The update of the daemon at each start of the app (HMD-055, R-NEW-updateall), and what the
//! page is told of it: the update, once, with both versions; a failed update, once, with its
//! step; and, while it lasts, an installed daemon that stays older than the app.
//!
//! I/O: runs the update off the async runtime, asks the runner of the new daemon for its
//! version, and keeps the notice of the last update in a file of the data folder of the app.

use std::{
    fs,
    path::{Path, PathBuf},
    sync::Mutex,
    time::{Duration, Instant},
};

use agent_threads::control::{Action, Outcome};
use anima_cli::agents::runner::RunnerLink;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager as _, Runtime, State, Webview};
use tauri_plugin_log::log;

use super::{
    DaemonError, Installing, carried, from_main, home_folder, placement, run, signature, this_user,
    update,
    version::{self, Version},
};

/// The file of the notice of the last update, in the data folder of the app.
const NOTICE_FILE: &str = "daemon-update.json";
/// How long the new daemon has to answer with its version after its start.
const HEALTH_LIMIT: Duration = Duration::from_secs(30);
/// The new daemon must still answer this long after its first answer: one that fails at its
/// token ends, and its unit starts it again and again.
const STEADY: Duration = Duration::from_secs(3);
/// The time of one try to reach the runner.
const TRY: Duration = Duration::from_secs(2);
/// The deadline of `--version` of the installed copy.
const VERSION_DEADLINE: Duration = Duration::from_secs(10);

/// What the page says of the daemon of this computer and the app.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Notice {
    /// The update replaced the daemon: `from` is `None` for a copy that printed no version.
    Updated { from: Option<String>, to: String },
    /// The update did not finish; `step` is the code of its error. The copy of before runs.
    NotUpdated {
        from: Option<String>,
        to: String,
        step: String,
    },
    /// The installed daemon is older than the app and stays: `why` says why the app did not
    /// replace it (`not_sealed`, or the code of the error of the update).
    Older {
        installed: Option<String>,
        app: String,
        why: String,
    },
}

/// The notice of the last update while it was not shown, and the older daemon while it lasts.
#[derive(Default)]
pub struct Notices(Mutex<Option<Notice>>);

/// The notice in the file, and whether the page was given it.
#[derive(Serialize, Deserialize)]
struct Kept {
    notice: Notice,
    shown: bool,
}

/// Replaces an installed copy of the daemon that is older than the one this app carries and
/// starts its unit again, off the async runtime. It holds the one install while it runs; when an
/// install or a removal already runs, it does nothing at this start.
pub fn update_at_start<R: Runtime>(app: &AppHandle<R>) {
    let (Ok(carried), Ok(home)) = (carried(), home_folder()) else {
        return;
    };
    let Ok(claim) = app.state::<Installing>().claim() else {
        log::info!("the daemon was not compared with the one of the app: an install runs");
        return;
    };
    let kept = app
        .path()
        .app_local_data_dir()
        .ok()
        .map(|data| data.join(NOTICE_FILE));
    let app = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let copy = placement::destination(&home);
        let env = run::daemon_env(&home, |name| std::env::var_os(name));
        let read = |program: &Path| version::read(program, &env, VERSION_DEADLINE, &claim.stop);
        let (installed, offered) = (read(&copy), read(&carried));
        if signature::of_program(&carried) == signature::Identity::NewEachBuild {
            log::info!(
                "this build has no Developer ID signature: after an update, the Keychain asks the person once for the new daemon"
            );
        }
        let swap = update::Swap {
            copy: &copy,
            carried: &carried,
            user: this_user(),
            env: &env,
            restart: super::INSTALL_DEADLINE,
            stop: &claim.stop,
            sealed: bundle_of(&carried).is_some_and(|bundle| signature::sealed(&bundle)),
            signed: &signature::sealed,
            healthy: &answers,
        };
        let result = update::update_here(&swap);
        log_result(result);
        let Some(offered) = offered else {
            return;
        };
        let (once, lasting) = notices(result, installed, offered);
        if let (Some(once), Some(kept)) = (once, kept.as_deref()) {
            keep(kept, &once);
        }
        *app.state::<Notices>()
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = lasting;
    });
}

/// The notice to show once and the one that lasts, from what the update did.
fn notices(
    result: Result<update::Updated, DaemonError>,
    installed: Option<Version>,
    offered: Version,
) -> (Option<Notice>, Option<Notice>) {
    let (from, to) = (installed.map(text), text(offered));
    let older = |why: &str| Notice::Older {
        installed: from.clone(),
        app: to.clone(),
        why: why.to_owned(),
    };
    match result {
        Ok(update::Updated::Replaced) => (
            Some(Notice::Updated {
                from: from.clone(),
                to: to.clone(),
            }),
            None,
        ),
        Ok(update::Updated::NotSealed) => (None, Some(older("not_sealed"))),
        Ok(_) => (None, None),
        Err(error) => (
            Some(Notice::NotUpdated {
                from: from.clone(),
                to: to.clone(),
                step: error.code().to_owned(),
            }),
            Some(older(error.code())),
        ),
    }
}

fn text([major, minor, patch]: Version) -> String {
    format!("{major}.{minor}.{patch}")
}

fn log_result(result: Result<update::Updated, DaemonError>) {
    match result {
        Ok(update::Updated::Replaced) => {
            log::info!("the daemon on this computer was replaced by the newer one of the app");
        }
        Ok(update::Updated::KeptNewer) => {
            log::info!("the daemon on this computer is newer than the one of the app; it stays");
        }
        Ok(update::Updated::CarriedUnknown) => {
            log::warn!("the version of the daemon of the app could not be read; no update");
        }
        Ok(update::Updated::NotSealed) => {
            log::warn!(
                "the daemon on this computer is older than the one of the app, but the seal of the app does not verify: no update"
            );
        }
        Ok(update::Updated::NoCopy | update::Updated::Unchanged) => {}
        Err(error) => log::warn!("the daemon was not updated: {}", error.code()),
    }
}

/// The app bundle that holds `carried` (`<bundle>.app/Contents/MacOS/anima-daemon`); on a
/// system with no bundles, the folder of the app.
fn bundle_of(carried: &Path) -> Option<PathBuf> {
    let folder = carried.parent()?;
    if cfg!(target_os = "macos") {
        let bundle = folder.parent()?.parent()?;
        return (bundle.extension()? == "app").then(|| bundle.to_owned());
    }
    Some(folder.to_owned())
}

/// Whether the runner of the daemon answers with `version`, two times [`STEADY`] apart, within
/// [`HEALTH_LIMIT`]. A runner older than the call closes it, so it never counts.
fn answers(version: Version) -> bool {
    tauri::async_runtime::block_on(async {
        let end = Instant::now() + HEALTH_LIMIT;
        let mut answered = false;
        while Instant::now() < end {
            if runner_version().await == Some(version) {
                if answered {
                    return true;
                }
                answered = true;
                tokio::time::sleep(STEADY).await;
            } else {
                answered = false;
                tokio::time::sleep(Duration::from_millis(500)).await;
            }
        }
        false
    })
}

/// The version that the runner of this computer names, when it answers within [`TRY`].
async fn runner_version() -> Option<Version> {
    let asked = async {
        let mut link = RunnerLink::connect().await.ok()?;
        match link.call(Action::Version).await.ok()? {
            Outcome::Version { version } => {
                version::parse(format!("anima-daemon {version}").as_bytes())
            }
            _ => None,
        }
    };
    tokio::time::timeout(TRY, asked).await.ok().flatten()
}

/// Writes `notice` as not shown yet, readable by this user only.
fn keep(file: &Path, notice: &Notice) {
    let kept = Kept {
        notice: notice.clone(),
        shown: false,
    };
    if write(file, &kept).is_err() {
        log::warn!("the notice of the daemon update could not be written");
    }
}

fn write(file: &Path, kept: &Kept) -> std::io::Result<()> {
    use std::{io::Write as _, os::unix::fs::OpenOptionsExt as _};
    let folder = file.parent().ok_or(std::io::ErrorKind::NotFound)?;
    fs::create_dir_all(folder)?;
    let part = folder.join(format!("{NOTICE_FILE}.part"));
    let _ = fs::remove_file(&part);
    let mut out = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&part)?;
    out.write_all(&serde_json::to_vec(kept).map_err(std::io::Error::other)?)?;
    out.sync_all()?;
    fs::rename(&part, file)
}

/// The notices for the page: the last update once, then the older daemon while it lasts.
#[tauri::command]
pub async fn daemon_update_notice<R: Runtime>(
    webview: Webview<R>,
    notices: State<'_, Notices>,
) -> Result<Vec<Notice>, DaemonError> {
    from_main(webview.label())?;
    let mut shown = Vec::new();
    if let Ok(file) = webview
        .path()
        .app_local_data_dir()
        .map(|data| data.join(NOTICE_FILE))
        && let Some(kept) = fs::read(&file)
            .ok()
            .and_then(|bytes| serde_json::from_slice::<Kept>(&bytes).ok())
        && !kept.shown
    {
        let notice = kept.notice.clone();
        if write(
            &file,
            &Kept {
                shown: true,
                ..kept
            },
        )
        .is_ok()
        {
            shown.push(notice);
        }
    }
    let lasting = notices
        .0
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone();
    shown.extend(lasting);
    Ok(shown)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hmd_055_an_update_is_told_once_and_a_failed_one_also_while_the_daemon_stays_older() {
        let (once, lasting) = notices(Ok(update::Updated::Replaced), Some([0, 9, 29]), [0, 10, 72]);
        assert_eq!(
            once,
            Some(Notice::Updated {
                from: Some("0.9.29".to_owned()),
                to: "0.10.72".to_owned()
            })
        );
        assert_eq!(lasting, None);
        let (once, lasting) = notices(Err(DaemonError::Unhealthy), None, [0, 10, 72]);
        assert!(matches!(once, Some(Notice::NotUpdated { step, .. }) if step == "unhealthy"));
        assert!(matches!(lasting, Some(Notice::Older { why, .. }) if why == "unhealthy"));
        let (once, lasting) = notices(
            Ok(update::Updated::NotSealed),
            Some([0, 9, 29]),
            [0, 10, 72],
        );
        assert_eq!(once, None);
        assert!(matches!(lasting, Some(Notice::Older { why, .. }) if why == "not_sealed"));
        assert_eq!(
            notices(
                Ok(update::Updated::Unchanged),
                Some([0, 10, 72]),
                [0, 10, 72]
            ),
            (None, None)
        );
    }
}
