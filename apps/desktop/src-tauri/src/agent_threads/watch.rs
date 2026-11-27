//! One native folder watch per shell, with bounded notification buffering (THR-058).
use super::{ThreadsError, registrations::Registrations};
use notify::{Config, RecommendedWatcher, Watcher};
use std::{
    path::Path,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    time::Duration,
};
use tauri::{AppHandle, Emitter, EventTarget, Runtime, State, Webview};

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
enum Change {
    Ready,
    Changed,
    Unavailable,
}

#[derive(Debug, thiserror::Error)]
enum WatchError {
    #[error("session watch filesystem failure")]
    Filesystem(#[from] std::io::Error),
    #[error("session watch backend failure")]
    Native(#[from] notify::Error),
    #[error("session watch stopped unexpectedly")]
    Ended,
}

const EVENT: &str = "agent-threads:changed";
const QUIET: Duration = Duration::from_millis(250);

#[derive(Default)]
pub struct Watching(Mutex<Option<Arc<AtomicBool>>>);

impl Watching {
    pub fn stop(&self) {
        let mut active = self
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(running) = active.take() {
            running.store(false, Ordering::Release);
        }
    }
}

/// Idempotent across page mounts. Failure leaves the client's interval available.
#[expect(
    clippy::needless_pass_by_value,
    reason = "Tauri commands take their arguments by value"
)]
#[tauri::command]
pub fn agent_threads_watch<R: Runtime>(
    webview: Webview<R>,
    app: AppHandle<R>,
    state: State<'_, Watching>,
) -> Result<(), ThreadsError> {
    crate::web_view::caller(webview.label()).map_err(|_| ThreadsError::Caller)?;
    let mut active = state.0.lock().map_err(|_| ThreadsError::Failed)?;
    if active
        .as_ref()
        .is_some_and(|running| running.load(Ordering::Acquire))
    {
        app.emit_to(
            EventTarget::webview(crate::web_view::MAIN),
            EVENT,
            Change::Ready,
        )
        .map_err(|_| ThreadsError::Failed)?;
        return Ok(());
    }
    let home = anima_cli::agents::threads::home().map_err(|_| ThreadsError::Home)?;
    let running = Arc::new(AtomicBool::new(true));
    let worker = Arc::clone(&running);
    std::thread::Builder::new()
        .name("session-folders".into())
        .spawn(move || {
            let healthy = watch(&home, &worker, |healthy| {
                app.emit_to(EventTarget::webview(crate::web_view::MAIN), EVENT, healthy)
                    .is_ok()
            })
            .is_ok();
            worker.store(false, Ordering::Release);
            if !healthy {
                tauri_plugin_log::log::warn!("session folder watch unavailable; using interval");
                if app
                    .emit_to(
                        EventTarget::webview(crate::web_view::MAIN),
                        EVENT,
                        Change::Unavailable,
                    )
                    .is_err()
                {
                    tauri_plugin_log::log::warn!("session folder watch status could not be sent");
                }
            }
        })
        .map_err(|_| ThreadsError::Failed)?;
    *active = Some(running);
    Ok(())
}

fn watch(
    home: &Path,
    running: &AtomicBool,
    changed: impl Fn(Change) -> bool,
) -> Result<(), WatchError> {
    let home = home.canonicalize()?;
    let (send, receive) = mpsc::sync_channel(1);
    let failed = Arc::new(AtomicBool::new(false));
    let broken = Arc::clone(&failed);
    let mut watcher = RecommendedWatcher::new(
        move |event: notify::Result<notify::Event>| {
            let relevant = if let Ok(event) = event {
                !event.kind.is_access()
            } else {
                broken.store(true, Ordering::Release);
                true
            };
            if relevant {
                match send.try_send(()) {
                    Ok(()) | Err(mpsc::TrySendError::Full(())) => (),
                    Err(mpsc::TrySendError::Disconnected(())) => {
                        broken.store(true, Ordering::Release);
                    }
                }
            }
        },
        Config::default().with_follow_symlinks(false),
    )?;
    let mut registrations = Registrations::default();
    registrations.reconcile(&mut watcher, &home)?;
    if !changed(Change::Ready) {
        return Ok(());
    }
    while running.load(Ordering::Acquire) {
        match receive.recv_timeout(QUIET) {
            Ok(()) => {
                std::thread::sleep(QUIET);
                if failed.load(Ordering::Acquire) {
                    return Err(WatchError::Ended);
                }
                match receive.try_recv() {
                    Ok(()) | Err(mpsc::TryRecvError::Empty) => (),
                    Err(mpsc::TryRecvError::Disconnected) => return Err(WatchError::Ended),
                }
                registrations.reconcile(&mut watcher, &home)?;
                if !running.load(Ordering::Acquire) || !changed(Change::Changed) {
                    break;
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => (),
            Err(mpsc::RecvTimeoutError::Disconnected) => return Err(WatchError::Ended),
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
