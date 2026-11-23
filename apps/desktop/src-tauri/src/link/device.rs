//! The `anima-link device` child (2.6): `-stop-with-stdin` ends it with the app, even a killed app.
//! A port that another program holds is reported, never changed (F-2); no line of the child's log is logged (LOG-007).

use std::{
    ffi::{OsStr, OsString},
    io::{BufRead, BufReader, Read},
    net::{Ipv4Addr, TcpListener},
    path::PathBuf,
    process::{Child, ChildStderr, Command, Stdio},
    sync::{
        Arc, Mutex, PoisonError,
        atomic::{AtomicBool, AtomicU32, Ordering},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

use super::{
    printed::{self, DeviceLine},
    restarts::Restarts,
    state::{DeviceState, Event, Reason},
};

const POLL: Duration = Duration::from_millis(50);
pub const WATCH_EVERY: Duration = Duration::from_secs(2);
pub const STOP_PATIENCE: Duration = Duration::from_secs(3);
const MOST_LINE: u64 = 4096;

#[derive(Debug, Clone)]
pub struct Launch {
    pub program: PathBuf,
    pub folder: PathBuf,
    pub port: u16,
    pub env: Vec<(&'static str, OsString)>,
    pub daemon_socket: Option<PathBuf>,
    pub restart_after: Duration,
}

#[derive(Debug, Default)]
pub struct DeviceLink {
    state: Arc<Mutex<DeviceState>>,
    running: Mutex<Option<Running>>,
    /// Set once the hub is a hosted one: a unit of the user then keeps the link running with the app closed (HMD-058).
    background: AtomicBool,
    pid: Arc<AtomicU32>,
}

#[derive(Debug)]
struct Running {
    stop: Arc<AtomicBool>,
    thread: JoinHandle<()>,
}

impl DeviceLink {
    #[cfg(windows)]
    pub fn child_pid(&self) -> u32 {
        self.pid.load(Ordering::Acquire)
    }

    pub fn state(&self) -> DeviceState {
        *self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    pub fn keep_in_background(&self) {
        self.background.store(true, Ordering::Release);
    }

    pub fn in_background(&self) -> bool {
        self.background.load(Ordering::Acquire)
    }

    pub fn record(&self, event: Event) {
        record(&self.state, event);
    }

    pub fn start(&self, launch: Launch) {
        let mut running = self.running.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(before) = running.take() {
            before.end();
        }
        let stop = Arc::new(AtomicBool::new(false));
        let state = Arc::clone(&self.state);
        let stop_seen = Arc::clone(&stop);
        let pid = Arc::clone(&self.pid);
        record(&self.state, Event::Starting { port: launch.port });
        let thread = thread::spawn(move || supervise(&launch, &stop_seen, &state, &pid));
        *running = Some(Running { stop, thread });
    }

    /// No child is started, and [`Self::stop`] ends only the watch: the link runs on with the app closed (HMD-058).
    pub fn watch(&self, port: u16, serving: impl Fn() -> Option<u16> + Send + 'static) {
        let mut running = self.running.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(before) = running.take() {
            before.end();
        }
        let stop = Arc::new(AtomicBool::new(false));
        let state = Arc::clone(&self.state);
        let stop_seen = Arc::clone(&stop);
        let thread = thread::spawn(move || observe(port, &serving, &stop_seen, &state));
        *running = Some(Running { stop, thread });
    }

    /// Blocks: call it off the async runtime.
    pub fn stop(&self) {
        let before = self
            .running
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take();
        if let Some(before) = before {
            before.end();
        }
        record(&self.state, Event::Stopped);
    }
}

impl Running {
    fn end(self) {
        self.stop.store(true, Ordering::Release);
        let _ = self.thread.join();
    }
}

fn record(state: &Mutex<DeviceState>, event: Event) {
    let mut state = state.lock().unwrap_or_else(PoisonError::into_inner);
    *state = state.after(event);
}

fn supervise(launch: &Launch, stop: &AtomicBool, state: &Arc<Mutex<DeviceState>>, pid: &AtomicU32) {
    let mut restarts = Restarts::default();
    while !stop.load(Ordering::Acquire) {
        let began = Instant::now();
        record(state, Event::Starting { port: launch.port });
        run_once(launch, stop, state, pid);
        let port_taken =
            state.lock().unwrap_or_else(PoisonError::into_inner).reason == Some(Reason::PortTaken);
        if !stop.load(Ordering::Acquire) && !port_taken && !restarts.after(began.elapsed()) {
            tauri_plugin_log::log::warn!("device link stopped after repeated early exits");
            record(state, Event::Unable(Reason::RepeatedFailure));
            return;
        }
        let resume = Instant::now() + launch.restart_after;
        while !stop.load(Ordering::Acquire) && Instant::now() < resume {
            thread::sleep(POLL);
        }
    }
    record(state, Event::Stopped);
}

fn observe(
    port: u16,
    serving: &dyn Fn() -> Option<u16>,
    stop: &AtomicBool,
    state: &Mutex<DeviceState>,
) {
    record(state, Event::Starting { port });
    let mut up = false;
    while !stop.load(Ordering::Acquire) {
        match serving() {
            Some(reported) if !up => {
                record(state, Event::Starting { port });
                record(state, Event::Ready { port: reported });
                up = true;
            }
            None if up => {
                record(state, Event::Ended);
                up = false;
            }
            _ => {}
        }
        let next = Instant::now() + WATCH_EVERY;
        while !stop.load(Ordering::Acquire) && Instant::now() < next {
            thread::sleep(POLL);
        }
    }
    record(state, Event::Stopped);
}

fn run_once(launch: &Launch, stop: &AtomicBool, state: &Arc<Mutex<DeviceState>>, pid: &AtomicU32) {
    // A port another program holds is seen before the child would fail on it.
    if TcpListener::bind((Ipv4Addr::LOCALHOST, launch.port)).is_err() {
        record(state, Event::PortTaken);
        return;
    }
    let Ok(mut child) = start(launch) else {
        record(state, Event::Ended);
        return;
    };
    pid.store(child.id(), Ordering::Release);
    let reader = child.stderr.take().map(|stderr| {
        let state = Arc::clone(state);
        thread::spawn(move || read_log(stderr, &state))
    });
    wait(&mut child, stop);
    pid.store(0, Ordering::Release);
    if let Some(reader) = reader {
        let _ = reader.join();
    }
    record(state, Event::Ended);
}

fn start(launch: &Launch) -> std::io::Result<Child> {
    let listen = format!("127.0.0.1:{}", launch.port);
    let mut command = Command::new(&launch.program);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x0800_0000);
    }
    #[cfg(unix)]
    command.current_dir("/");
    #[cfg(windows)]
    command.current_dir(&launch.folder);
    command
        .arg("device")
        .arg("-state")
        .arg(&launch.folder)
        .args(["-listen", &listen, "-stop-with-stdin"])
        .args(
            launch
                .daemon_socket
                .iter()
                .flat_map(|socket| [OsStr::new("-daemon-socket"), socket.as_os_str()]),
        )
        .env_clear()
        .envs(launch.env.iter().map(|(name, value)| (name, value)))
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
}

fn wait(child: &mut Child, stop: &AtomicBool) {
    let mut stopping: Option<Instant> = None;
    loop {
        match child.try_wait() {
            Ok(Some(_)) => return,
            Ok(None) => {}
            Err(_) => break,
        }
        match stopping {
            None if stop.load(Ordering::Acquire) => {
                drop(child.stdin.take());
                stopping = Some(Instant::now());
            }
            Some(since) if since.elapsed() > STOP_PATIENCE => break,
            _ => {}
        }
        thread::sleep(POLL);
    }
    let _ = child.kill();
    let _ = child.wait();
}

fn read_log(stderr: ChildStderr, state: &Mutex<DeviceState>) {
    let mut log = BufReader::new(stderr);
    let mut line = Vec::new();
    loop {
        line.clear();
        match (&mut log).take(MOST_LINE).read_until(b'\n', &mut line) {
            Ok(0) | Err(_) => return,
            Ok(_) => {}
        }
        let event = match printed::device_line(&String::from_utf8_lossy(&line)) {
            Some(DeviceLine::Ready(port)) => Event::Ready { port },
            Some(DeviceLine::PortTaken) => Event::PortTaken,
            None => continue,
        };
        record(state, event);
    }
}
