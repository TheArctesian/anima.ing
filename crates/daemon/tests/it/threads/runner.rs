//! The runner of `anima-daemon threads` with a made `claude` on the `PATH` of a made login
//! shell: a start, a request and its answer, a restart of the runner while the program stays in
//! the holder, and a stop that ends a child in a group of its own (TH11, TH12, THR-030,
//! THR-049, THR-113).

use std::path::{Path, PathBuf};
use std::process::{Child, Command as Process};
use std::time::Duration;

use agent_threads::control::{Action, Call, Change, Outcome, Reply, RunState, SOCKET};
use anima_daemon::threads::tree;
use rustix::process::{Pid, Signal};
use tokio::io::{AsyncBufReadExt as _, AsyncWriteExt as _, BufReader, Lines};
use tokio::net::UnixStream;
use tokio::net::unix::{OwnedReadHalf, OwnedWriteHalf};

mod commands;
mod grok;
mod lifecycle;
mod made;
mod managed;
mod relay;

pub(super) use made::CLAUDE;
use made::{CODEX, SHELL};

/// A secret of Anima in the environment of the daemon.
pub(super) const SECRET: &str = "s3cret-value";

pub(super) struct Kit {
    home: tempfile::TempDir,
    runner: Option<Child>,
}

impl Kit {
    pub(super) fn new() -> Self {
        Self::with(CLAUDE)
    }

    /// A kit whose made Claude Code is `claude`.
    pub(super) fn with(claude: &str) -> Self {
        let mut kit = Self::unstarted(claude);
        kit.start();
        kit
    }

    /// A kit with no runner yet.
    pub(super) fn unstarted(claude: &str) -> Self {
        use std::os::unix::fs::PermissionsExt as _;
        // Keep Git records outside shared `/tmp` and socket paths short in deep checkouts.
        #[cfg(target_os = "linux")]
        let home = tempfile::tempdir_in("/var/tmp").expect("a folder");
        #[cfg(not(target_os = "linux"))]
        let home = tempfile::tempdir().expect("a folder");
        let bin = home.path().join("bin");
        std::fs::create_dir(&bin).expect("bin");
        for (name, text) in [("claude", claude), ("codex", CODEX), ("login-shell", SHELL)] {
            let path = bin.join(name);
            std::fs::write(&path, text).expect("written");
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).expect("mode");
        }
        std::fs::create_dir(home.path().join("work")).expect("work");
        Self { home, runner: None }
    }

    pub(super) fn path(&self, name: &str) -> PathBuf {
        self.home.path().join(name)
    }

    pub(super) fn threads(&self) -> PathBuf {
        self.path("daemon").join("threads")
    }

    pub(super) fn start(&mut self) {
        self.runner = Some(self.spawn_runner());
    }

    /// One more `anima-daemon threads` on the folder of this kit.
    pub(super) fn spawn_runner(&self) -> Child {
        self.spawn_runner_from(Path::new(env!("CARGO_BIN_EXE_anima-daemon")))
    }

    /// The runner of this kit from the program file at `daemon`, which also starts the holder.
    pub(super) fn start_from(&mut self, daemon: &Path) {
        self.runner = Some(self.spawn_runner_from(daemon));
    }

    fn spawn_runner_from(&self, daemon: &Path) -> Child {
        let bin = self.path("bin");
        Process::new(daemon)
            .arg("threads")
            .env_clear()
            .env("HOME", self.home.path())
            .env("ANIMA_DAEMON_HOME", self.path("daemon"))
            .env("SHELL", bin.join("login-shell"))
            .env("PATH", format!("{}:/usr/bin:/bin", bin.display()))
            .env("ANIMA_TEST_SECRET", SECRET)
            .spawn()
            .expect("the runner starts")
    }

    pub(super) fn end_runner(&mut self) {
        if let Some(mut child) = self.runner.take() {
            let pid = rustix::process::Pid::from_raw(i32::try_from(child.id()).expect("a pid"));
            let _ =
                rustix::process::kill_process(pid.expect("a pid"), rustix::process::Signal::TERM);
            let _ = child.wait();
        }
    }

    pub(super) async fn client(&self) -> Client {
        let socket = self.threads().join(SOCKET);
        for _ in 0..200 {
            if let Ok(stream) = UnixStream::connect(&socket).await {
                let (read, write) = stream.into_split();
                return Client {
                    lines: BufReader::new(read).lines(),
                    write,
                    next: 0,
                };
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        panic!("the runner did not listen");
    }
}

/// How long the holder may take to end its programs after `SIGTERM`: a grace of a few seconds
/// for each thread, on a busy computer.
const HOLDER_END: Duration = Duration::from_secs(30);

impl Drop for Kit {
    /// Ends the runner, then each holder of its folder with its programs. The holder outlives
    /// the runner (THR-113) and refuses a `Quit` while a thread runs, so it gets `SIGTERM`, which
    /// ends each program it holds (THR-049). The holders are found by their command line, not by
    /// the socket: on a busy computer a holder may not listen yet when a short test ends, and an
    /// older holder may have given the socket up (TH98). One missed would run on with its folder
    /// deleted and its made programs. A holder that still runs after the wait fails the test and
    /// is killed, so that a failing test leaves nothing behind either.
    fn drop(&mut self) {
        self.end_runner();
        let holders = holders_of(&self.threads());
        for pid in &holders {
            signal(*pid, Signal::TERM);
        }
        let deadline = std::time::Instant::now() + HOLDER_END;
        while holders.iter().any(|pid| tree::alive(*pid)) && std::time::Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(50));
        }
        // Looked up again, so that a process that took the ID of an ended holder is not killed.
        let left: Vec<u32> = holders_of(&self.threads())
            .into_iter()
            .filter(|pid| holders.contains(pid))
            .collect();
        for pid in &left {
            signal(*pid, Signal::KILL);
        }
        // A second panic while the test already fails would abort the whole test binary.
        assert!(
            std::thread::panicking() || left.is_empty(),
            "the holders {left:?} of the test still ran {HOLDER_END:?} after SIGTERM"
        );
    }
}

fn signal(pid: u32, signal: Signal) {
    if let Some(pid) = i32::try_from(pid).ok().and_then(Pid::from_raw) {
        let _ = rustix::process::kill_process(pid, signal);
    }
}

/// The processes that run `anima-daemon holder <dir>`, from `ps`; none when `ps` cannot run, as
/// a panic in a drop while the test already fails would abort each test of the binary.
pub(super) fn holders_of(dir: &Path) -> Vec<u32> {
    let Ok(output) = Process::new("/bin/ps")
        .args(["-axww", "-o", "pid=,command="])
        .output()
    else {
        return Vec::new();
    };
    let mark = format!(" holder {}", dir.display());
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter(|line| {
            let line = line.trim_end();
            line.strip_suffix(" --channel")
                .unwrap_or(line)
                .ends_with(&mark)
        })
        .filter_map(|line| line.split_whitespace().next()?.parse().ok())
        .collect()
}

pub(super) struct Client {
    lines: Lines<BufReader<OwnedReadHalf>>,
    write: OwnedWriteHalf,
    next: u64,
}

impl Client {
    pub(super) async fn call(&mut self, action: Action) -> u64 {
        self.next += 1;
        let mut text = serde_json::to_string(&Call {
            id: self.next,
            action,
        })
        .expect("a call");
        text.push('\n');
        self.write.write_all(text.as_bytes()).await.expect("sent");
        self.next
    }

    pub(super) async fn until(&mut self, mut wanted: impl FnMut(&Reply) -> bool) -> Reply {
        let wait = async {
            loop {
                let line = self.lines.next_line().await.expect("reads").expect("open");
                let reply: Reply = serde_json::from_str(&line).expect("a reply");
                if wanted(&reply) {
                    return reply;
                }
            }
        };
        tokio::time::timeout(Duration::from_secs(20), wait)
            .await
            .expect("in time")
    }

    pub(super) async fn done(&mut self, id: u64) -> Outcome {
        match self
            .until(|reply| matches!(reply, Reply::Done { id: done, .. } if *done == id))
            .await
        {
            Reply::Done { result, .. } => result,
            Reply::Update(_) => unreachable!("only a done is wanted"),
        }
    }
}

pub(super) fn change(reply: &Reply) -> Option<&Change> {
    match reply {
        Reply::Update(update) => Some(&update.change),
        Reply::Done { .. } => None,
    }
}

pub(super) fn state_is(state: RunState) -> impl FnMut(&Reply) -> bool {
    move |reply| matches!(change(reply), Some(Change::State { state: now, .. }) if *now == state)
}

pub(super) async fn child_pid(folder: &Path) -> u32 {
    for _ in 0..100 {
        if let Some(pid) = std::fs::read_to_string(folder.join("child.pid"))
            .ok()
            .and_then(|text| text.trim().parse().ok())
        {
            return pid;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    panic!("the made program wrote no child");
}
