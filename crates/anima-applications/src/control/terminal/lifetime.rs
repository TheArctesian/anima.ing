//! Own the exact exec process, including when the local relay disappears.
use std::{
    fs::File,
    io::{self, Read as _, Write as _},
    os::{
        fd::OwnedFd,
        unix::{net::UnixStream, process::CommandExt as _},
    },
    process::{Child, Command, Stdio},
    thread,
    time::{Duration, Instant},
};

use rustix::{
    event::{PollFd, PollFlags, Timespec, poll},
    io::Errno,
    process::{Pid, PidfdFlags, Signal, getppid, pidfd_open, pidfd_send_signal},
};

use super::{Error, Target, engine::Engine};

pub(super) const START_GATE: &str = r#"stty -icanon min 0 time 100 -echo || exit 1; IFS= read -r start && [ "$start" = open ] || exit 1; stty sane || exit 1; exec /bin/sh -i"#;
const HANDOFF: Duration = Duration::from_secs(5);

pub(super) struct Process(OwnedFd);

pub(super) fn nonce() -> Result<String, Error> {
    let mut bytes = [0_u8; 16];
    getrandom::fill(&mut bytes).map_err(|_| Error::Terminal(io::ErrorKind::Other))?;
    Ok(bytes
        .iter()
        .fold(String::with_capacity(32), |mut text, byte| {
            use std::fmt::Write as _;
            let _ = write!(text, "{byte:02x}");
            text
        }))
}

impl Process {
    pub(super) fn verify(
        engine: &Engine<'_>,
        exec_id: &str,
        target: &Target,
        nonce: &str,
    ) -> Result<Self, Error> {
        let until = Instant::now() + HANDOFF;
        loop {
            let observed = engine.inspect(exec_id)?;
            if observed.id != exec_id
                || observed.container != target.container
                || observed.exit_code.is_some()
            {
                return Err(Error::Terminal(io::ErrorKind::InvalidData));
            }
            if let Some(pid) = Pid::from_raw(observed.pid).filter(|_| observed.running) {
                let fd = pidfd_open(pid, PidfdFlags::empty())?;
                verify_bootstrap(pid, &target.container, nonce)?;
                let confirm = engine.inspect(exec_id)?;
                if confirm.id != exec_id
                    || confirm.container != target.container
                    || !confirm.running
                    || confirm.pid != observed.pid
                    || exited(&fd)?
                {
                    return Err(Error::Terminal(io::ErrorKind::InvalidData));
                }
                return Ok(Self(fd));
            }
            if Instant::now() >= until {
                return Err(Error::Terminal(io::ErrorKind::TimedOut));
            }
            thread::sleep(Duration::from_millis(25));
        }
    }

    pub(super) fn watch(&self) -> Result<Child, Error> {
        let (mut relay, observer) = UnixStream::pair()?;
        relay.set_read_timeout(Some(HANDOFF))?;
        let mut child = Command::new(std::env::current_exe()?)
            .arg("console-watch")
            .arg(std::process::id().to_string())
            .env_clear()
            .stdin(Stdio::from(self.0.try_clone()?))
            .stdout(Stdio::from(OwnedFd::from(observer)))
            .stderr(Stdio::null())
            .process_group(0)
            .spawn()?;
        let mut acknowledgment = [0];
        if relay.read_exact(&mut acknowledgment).is_err() || acknowledgment != [1] {
            let _ = child.kill();
            let _ = child.wait();
            return Err(Error::Terminal(io::ErrorKind::NotConnected));
        }
        Ok(child)
    }

    pub(super) fn end(&self) -> Result<(), Error> {
        kill(&self.0)?;
        let mut fds = [PollFd::new(&self.0, PollFlags::IN)];
        if poll(
            &mut fds,
            Some(&Timespec {
                tv_sec: 2,
                tv_nsec: 0,
            }),
        )? == 0
        {
            return Err(Error::Terminal(io::ErrorKind::TimedOut));
        }
        Ok(())
    }
}

impl Drop for Process {
    fn drop(&mut self) {
        let _ = kill(&self.0);
    }
}

fn verify_bootstrap(pid: Pid, container: &str, nonce: &str) -> Result<(), Error> {
    let path = format!("/proc/{}", pid.as_raw_nonzero());
    let mut command = Vec::new();
    File::open(format!("{path}/cmdline"))?
        .take(2049)
        .read_to_end(&mut command)?;
    let exact = [
        "/bin/sh",
        "-c",
        START_GATE,
        "anima-application-terminal",
        nonce,
        "",
    ]
    .join("\0");
    let mut cgroups = String::new();
    File::open(format!("{path}/cgroup"))?
        .take(8193)
        .read_to_string(&mut cgroups)?;
    let scope = format!("docker-{container}.scope");
    let member = cgroups.lines().any(|line| {
        line.strip_prefix("0::")
            .and_then(|path| path.rsplit('/').next())
            .is_some_and(|group| group == scope || group == container)
    });
    if command != exact.as_bytes() || cgroups.len() > 8192 || !member {
        return Err(Error::Terminal(io::ErrorKind::PermissionDenied));
    }
    Ok(())
}

pub(super) fn watch_relay(expected_parent: Pid) -> Result<(), Error> {
    let shell = std::io::stdin();
    if getppid() != Some(expected_parent) {
        return Err(Error::Terminal(io::ErrorKind::PermissionDenied));
    }
    let parent = pidfd_open(expected_parent, PidfdFlags::empty())?;
    if getppid() != Some(expected_parent) {
        return Err(Error::Terminal(io::ErrorKind::NotConnected));
    }
    std::io::stdout().write_all(&[1])?;
    std::io::stdout().flush()?;
    let mut fds = [
        PollFd::new(&parent, PollFlags::IN),
        PollFd::new(&shell, PollFlags::IN),
    ];
    loop {
        match poll(&mut fds, None) {
            Err(Errno::INTR) => (),
            Err(error) => return Err(error.into()),
            Ok(_) => break,
        }
    }
    if !fds[0].revents().is_empty() {
        kill(&shell)?;
    }
    Ok(())
}

fn exited(fd: &OwnedFd) -> Result<bool, Error> {
    let mut fds = [PollFd::new(fd, PollFlags::IN)];
    Ok(poll(
        &mut fds,
        Some(&Timespec {
            tv_sec: 0,
            tv_nsec: 0,
        }),
    )? > 0)
}

fn kill(fd: impl std::os::fd::AsFd) -> Result<(), Error> {
    match pidfd_send_signal(fd, Signal::KILL) {
        Ok(()) | Err(Errno::SRCH) => Ok(()),
        Err(error) => Err(error.into()),
    }
}
