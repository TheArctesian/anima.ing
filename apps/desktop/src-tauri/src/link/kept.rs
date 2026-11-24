//! HMD-058, R-240. The link program is copied to a folder that the app makes in its own folder (not
//! `~/.local/bin`, whose mode the person sets). No rule of the unit is here: `link/unit` has them.

use std::{
    ffi::OsString,
    fs,
    path::{Path, PathBuf},
    time::Duration,
};

use std::{
    sync::{Arc, Mutex, PoisonError},
    thread,
    time::Instant,
};

use tauri::{Manager, Runtime, Webview};
use tauri_plugin_log::log;

use super::copy::{self as copies, current_copy, destination, earlier_copy, place, same, unplace};
use super::{LinkError, device::DeviceLink, folder, folders, launch, printed, repair::Repair};
use crate::carried::run::{Invocation, run};
#[cfg(unix)]
use unix::socket_present;
#[cfg(windows)]
#[path = "kept/windows.rs"]
mod windows;
#[cfg(windows)]
pub use windows::owns_background;
#[cfg(windows)]
use windows::{environment, socket_present};
#[cfg(unix)]
#[path = "kept/unix.rs"]
mod unix;
#[cfg(any(target_os = "macos", all(unix, test)))]
pub use unix::daemon_folder;
#[cfg(all(unix, test))]
pub use unix::private_socket;
#[cfg(unix)]
pub use unix::{environment, socket_of_user};

/// `unit install` runs up to three steps of 30 s each, one of them twice.
const UNIT_LIMIT: Duration = Duration::from_secs(150);
const ASK_LIMIT: Duration = Duration::from_secs(3);
const READY_LIMIT: Duration = Duration::from_secs(20);
const READY_POLL: Duration = Duration::from_millis(250);

#[derive(Debug, Clone)]
pub struct Kept {
    pub program: PathBuf,
    pub folder: PathBuf,
    pub port: u16,
    /// HMD-054; none when its folder cannot be a private one.
    pub daemon_socket: Option<PathBuf>,
    pub env: Vec<(&'static str, OsString)>,
}

const DAEMON_SOCKET: &str = "link.sock";
/// The longest path that the link takes for a socket.
const LONGEST_SOCKET: usize = 100;

fn ask(kept: &Kept, args: &[&str], limit: Duration) -> Option<Vec<u8>> {
    let args: Vec<OsString> = args.iter().map(OsString::from).collect();
    let args: Vec<&OsString> = args.iter().collect();
    let invocation = Invocation {
        program: &kept.program,
        args: &args,
        env: &kept.env,
        limit,
    };
    run(&invocation).ok()
}

impl Kept {
    pub fn install(&self) -> Result<(), LinkError> {
        let folder = self.folder.to_str().ok_or(LinkError::Folder)?;
        let listen = format!("127.0.0.1:{}", self.port);
        let mut args = vec!["unit", "install", "-state", folder, "-listen", &listen];
        if let Some(socket) = self.daemon_socket.as_deref().and_then(Path::to_str) {
            args.extend(["-daemon-socket", socket]);
        }
        let args: Vec<OsString> = args.iter().map(OsString::from).collect();
        let args: Vec<&OsString> = args.iter().collect();
        let invocation = Invocation {
            program: &self.program,
            args: &args,
            env: &self.env,
            limit: UNIT_LIMIT,
        };
        run(&invocation).map(drop).map_err(|cause| {
            // The first line of the link's refusal names a folder or a value, never a secret.
            log::warn!("`anima-link unit install` did not install the unit: {cause}");
            LinkError::Failed
        })
    }

    fn with_program(&self, program: PathBuf) -> Self {
        Self {
            program,
            ..self.clone()
        }
    }

    pub fn remove(&self) -> Result<(), LinkError> {
        ask(self, &["unit", "remove"], UNIT_LIMIT)
            .map(drop)
            .ok_or(LinkError::Failed)
    }

    /// The unit's file and label are the same for each copy of the app, so a unit of a different
    /// folder
    /// is never changed or removed from here.
    pub fn keeps_this_folder(&self) -> bool {
        let Some(folder) = self.folder.to_str() else {
            return false;
        };
        ask(self, &["unit", "show"], ASK_LIMIT)
            .and_then(|shown| printed::unit_file(&shown))
            .and_then(|file| fs::read_to_string(file).ok())
            .is_some_and(|unit| printed::unit_runs_folder(&unit, folder))
    }

    /// Each removal by the app has its line in the log.
    pub fn remove_because(&self, reason: &'static str) -> Result<(), LinkError> {
        let result = self.remove();
        match &result {
            Ok(()) => log::warn!("the unit of the device link was removed: {reason}"),
            Err(_) => log::warn!("the unit of the device link was not removed ({reason})"),
        }
        result
    }

    pub fn serving(&self) -> Option<u16> {
        let folder = self.folder.to_str()?;
        let status = ask(self, &["ctl", "-state", folder, "status"], ASK_LIMIT)?;
        printed::link_local(&status)
    }

    fn has_daemon_socket(&self) -> bool {
        self.daemon_socket.as_deref().is_some_and(socket_present)
    }

    /// Restarts the unit when the program changed or the unit predates the daemon socket (HMD-054).
    /// A copy that cannot be written stays, and the unit runs on.
    pub fn refresh(self, carried: &Path, application: &Path) -> Self {
        let copy = destination(application);
        let changed = self.program != copy || !same(carried, &self.program);
        if !changed {
            let restart = self.daemon_socket.is_some() && !self.has_daemon_socket();
            if (restart || copies::waits_for_restart(&self.program)) && self.install().is_ok() {
                copies::forget_previous(&self.program);
            }
            return self;
        }
        #[cfg(windows)]
        if self.remove().is_err() {
            return self;
        }
        let Ok(placed) = place(carried, application) else {
            #[cfg(windows)]
            let _ = self.install();
            return self;
        };
        let moved = self.with_program(placed);
        if moved.install().is_err() {
            #[cfg(windows)]
            let _ = self.install();
            return self;
        }
        if self.program != moved.program {
            unplace(&self.program);
        }
        copies::forget_previous(&moved.program);
        moved
    }

    pub fn put_back(&self, carried: Option<&Path>, application: Option<&Path>) -> bool {
        if !folder::wants_unit(&self.folder) {
            return false;
        }
        if self.program.is_file() {
            return self.install().is_ok();
        }
        carried
            .zip(application)
            .and_then(|(carried, application)| place(carried, application).ok())
            .is_some_and(|placed| self.with_program(placed).install().is_ok())
    }

    pub fn watch(self, device: &DeviceLink) {
        let port = self.port;
        let repair = Mutex::new(Repair::default());
        device.watch(port, move || {
            let serving = self.serving();
            let due = repair
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .due(serving.is_some());
            if due {
                let carried = crate::carried::program(super::PROGRAM);
                let application = crate::user_folders::of_this_user();
                if self.put_back(carried.as_deref(), application.as_deref()) {
                    log::warn!(
                        "the device link of the unit did not answer: the unit was installed and started again"
                    );
                } else {
                    log::warn!(
                        "the device link of the unit did not answer, and the unit could not be put back"
                    );
                }
            }
            serving
        });
    }
}

fn at(folder: &Path, port: u16) -> Result<Kept, LinkError> {
    let home = crate::user_folders::home().ok_or(LinkError::NoHome)?;
    let application = crate::user_folders::of_this_user().ok_or(LinkError::NoHome)?;
    #[cfg(unix)]
    let daemon_socket = socket_of_user(&home);
    #[cfg(windows)]
    let daemon_socket = windows::private_socket();
    Ok(Kept {
        program: current_copy(&application, &home),
        folder: folder.to_path_buf(),
        port,
        daemon_socket,
        env: environment(|name| std::env::var_os(name)),
    })
}

pub fn now(folder: &Path, port: u16) -> Option<Kept> {
    at(folder, port)
        .ok()
        .filter(|link| link.program.is_file() && link.keeps_this_folder())
}

pub fn daemon_socket(port: u16) -> Option<PathBuf> {
    let folders = folders().ok()?;
    if folder::read_port(&folders.device) != Some(port) {
        return None;
    }
    let link = now(&folders.device, port)?;
    link.has_daemon_socket()
        .then_some(link.daemon_socket)
        .flatten()
}

fn answers(link: &Kept, limit: Duration) -> bool {
    let end = Instant::now() + limit;
    loop {
        if link.serving() == Some(link.port) {
            return true;
        }
        if Instant::now() >= end {
            return false;
        }
        thread::sleep(READY_POLL);
    }
}

/// HMD-058. A unit whose link does not answer is removed again.
fn keep(device: &DeviceLink, program: &Path, folder: &Path, port: u16) -> Result<(), &'static str> {
    let home = crate::user_folders::home().ok_or("no home folder")?;
    let application = crate::user_folders::of_this_user().ok_or("no home folder")?;
    let placed = place(program, &application)
        .map_err(|_| "the link program was not copied to the folder of the link")?;
    let link = Kept {
        program: placed,
        ..at(folder, port).map_err(|_| "no home folder")?
    };
    #[cfg(unix)]
    if link.daemon_socket.is_none() {
        log::warn!("the folder of the daemon is not a private one: the unit gets no socket for it");
    }
    device.stop();
    if link.install().is_err() {
        let _ = link.remove_because("`anima-link unit install` failed");
        return Err("`anima-link unit install` failed");
    }
    if !answers(&link, READY_LIMIT) {
        let _ = link.remove_because("its link did not answer on its port");
        return Err("the link of the unit did not answer on its port");
    }
    let earlier = earlier_copy(&home);
    if earlier != link.program {
        unplace(&earlier);
    }
    link.watch(device);
    Ok(())
}

pub fn keep_or_run(device: &DeviceLink, program: PathBuf, folder: &Path, port: u16) {
    match keep(device, &program, folder, port) {
        Ok(()) => log::info!("a unit of the user keeps the device link"),
        Err(step) => {
            log::warn!("the device link was not handed to a unit of the user: {step}");
            device.start(launch(program, folder, port));
        }
    }
}

pub fn hand_over(device: &DeviceLink, program: PathBuf, folder: &Path) {
    let Some(port) = folder::read_port(folder) else {
        log::warn!("the device link has no saved port: no unit of the user is made");
        return;
    };
    if now(folder, port).is_none() {
        log::info!("the device link goes to a unit of the user");
        keep_or_run(device, program, folder, port);
    }
}

/// The sign-in of that moment, which uses the link, ends before the hand-over.
const HAND_OVER_AFTER: Duration = Duration::from_secs(45);

/// HMD-058: from now a unit of the user keeps the link of this device. With `joined`, the hand-over
/// waits so that a sign-in of now is not cut; if the app stops first, its next start hands over.
pub fn hosted_hub_opened<R: Runtime>(webview: &Webview<R>, joined: bool) {
    let device = Arc::clone(&webview.state::<Arc<DeviceLink>>());
    let first = !device.in_background();
    device.keep_in_background();
    let Ok(folders) = folders() else { return };
    if !joined {
        return;
    }
    if folder::want_unit(&folders.device).is_err() {
        log::warn!("the folder of the device link could not be marked for a unit");
    }
    if first {
        log::info!("this hub is a hosted one: a unit of the user will keep the device link");
    }
    // Once for each run of the app: a second answer of the cloud starts no second hand-over.
    if let Some(program) = crate::carried::program(super::PROGRAM).filter(|_| first) {
        thread::spawn(move || {
            thread::sleep(HAND_OVER_AFTER);
            hand_over(&device, program, &folders.device);
        });
    }
}
