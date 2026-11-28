use std::{
    fs,
    io::Write as _,
    os::{fd::OwnedFd, unix::ffi::OsStrExt as _},
    path::{Path, PathBuf},
    process::{Child, Command, Output, Stdio},
    time::{Duration, Instant},
};

use rustix::{
    event::{PollFd, PollFlags, Timespec, poll},
    process::{Pid, PidfdFlags, Signal, getgid, getgroups, getuid, pidfd_open, pidfd_send_signal},
};
use serde_json::json;

const BIN: &str = env!("CARGO_BIN_EXE_anima-daemon");
const SUPERVISE: &str = "linux-supervise";
const BOOTSTRAP: &[u8] = b"linux-protected-role";
const END: Duration = Duration::from_secs(10);

#[derive(Clone, Copy, Debug)]
pub(in super::super) enum Role {
    Daemon,
    Holder,
}

impl Role {
    const fn argument(self) -> &'static [u8] {
        match self {
            Self::Daemon => b"daemon",
            Self::Holder => b"holder",
        }
    }
}

pub(in super::super) struct RoleProcess {
    pid: Pid,
    fd: OwnedFd,
}

impl RoleProcess {
    pub(in super::super) fn id(&self) -> u32 {
        u32::try_from(self.pid.as_raw_pid()).expect("the role PID")
    }

    pub(in super::super) fn signal(&self, signal: Signal) {
        pidfd_send_signal(&self.fd, signal).expect("signal the exact protected role");
    }

    fn alive(&self) -> bool {
        poll(
            &mut [PollFd::new(&self.fd, PollFlags::IN)],
            Some(&Timespec::default()),
        )
        .is_ok_and(|count| count == 0)
    }
}

pub(super) struct Installation {
    dir: PathBuf,
}

impl Installation {
    pub(super) fn new(home: &Path, state: &Path, origin: &str, certificate: &str) -> Self {
        let uid = getuid();
        assert!(
            !uid.is_root(),
            "the Linux fixture needs a non-root test user"
        );
        // The fixture executes a root-owned ELF, and some runners mount /run with noexec.
        let dir = output(
            sudo("/usr/bin/mktemp").args(["-d", "/opt/anima-th32.XXXXXXXXXX"]),
            "create the isolated root fixture directory",
        );
        let installed = Self {
            dir: PathBuf::from(stdout(dir).trim()),
        };
        output(
            sudo("/bin/chmod").arg("0755").arg(&installed.dir),
            "make the fixture directory traversable",
        );
        output(
            sudo("/usr/bin/install")
                .args(["-m", "0755", "--", BIN])
                .arg(installed.binary()),
            "install the ordinary daemon ELF",
        );
        let name = stdout(output(Command::new("/usr/bin/id").arg("-un"), "login name"));
        let supplementary: Vec<_> = getgroups()
            .expect("the login groups")
            .into_iter()
            .map(rustix::process::Gid::as_raw)
            .collect();
        let config = json!({
            "uid": uid.as_raw(), "gid": getgid().as_raw(), "supplementary": supplementary,
            "home": home, "state": state, "shell": home.join("bin/login-shell"),
            "name": name.trim(), "authority": installed.dir.join("authority"),
            "hub_trust": { "origin": origin, "certificate": certificate },
        });
        let mut file = tempfile::NamedTempFile::new().expect("temporary fixture configuration");
        file.write_all(config.to_string().as_bytes())
            .expect("write the fixture configuration");
        output(
            sudo("/usr/bin/install")
                .args(["-m", "0600", "--"])
                .arg(file.path())
                .arg(installed.config()),
            "install the root supervisor configuration",
        );
        installed
    }

    fn binary(&self) -> PathBuf {
        self.dir.join("anima-daemon")
    }

    fn config(&self) -> PathBuf {
        self.dir.join("person.json")
    }

    pub(super) fn inherited_descriptor_path(&self) -> PathBuf {
        self.dir.join("inherited-descriptor")
    }

    pub(super) fn command(&self) -> Command {
        let mut launch = sudo(self.binary());
        launch.arg(SUPERVISE).arg(self.config());
        launch
    }

    pub(super) fn start(&self) -> Supervisor {
        let pid_file = self.dir.join("supervisor.pid");
        let child = sudo("/bin/sh")
            .args([
                "-c",
                "umask 077; printf '%s\\n' 'TH32 synthetic descriptor' > \"$1\" && exec 9<\"$1\" && shift &&
                 umask 022 && printf '%s\\n' \"$$\" > \"$1\" && shift && exec \"$@\"",
                "anima-th32",
            ])
            .arg(self.inherited_descriptor_path())
            .arg(&pid_file)
            .arg(self.binary())
            .arg(SUPERVISE)
            .arg(self.config())
            .stdin(Stdio::null())
            .spawn()
            .expect("start the root supervisor");
        Supervisor {
            child,
            pid_file,
            binary: self.binary(),
            config: self.config(),
        }
    }
}

impl Drop for Installation {
    fn drop(&mut self) {
        let result = sudo("/bin/rm").args(["-rf", "--"]).arg(&self.dir).status();
        assert!(
            std::thread::panicking() || result.is_ok_and(|status| status.success()),
            "could not remove the root fixture directory"
        );
    }
}

pub(super) struct Supervisor {
    child: Child,
    pid_file: PathBuf,
    binary: PathBuf,
    config: PathBuf,
}

impl Supervisor {
    fn pid(&self) -> Option<Pid> {
        let pid = parse_pid(fs::read_to_string(&self.pid_file).ok()?.trim())?;
        let args = [
            self.binary.as_os_str().as_bytes(),
            SUPERVISE.as_bytes(),
            self.config.as_os_str().as_bytes(),
        ];
        command_is(pid, &args).then_some(pid)
    }

    pub(super) fn assert_inherited(&self, expected: &Path) {
        let pid = self.pid().expect("the root supervisor PID").as_raw_pid();
        let inherited = output(
            sudo("/usr/bin/readlink").arg(format!("/proc/{pid}/fd/9")),
            "read the root supervisor's synthetic descriptor",
        );
        assert_eq!(Path::new(stdout(inherited).trim()), expected);
    }

    pub(super) fn process(&self, role: Role) -> Option<RoleProcess> {
        let supervisor = self.pid()?.as_raw_pid();
        let children =
            fs::read_to_string(format!("/proc/{supervisor}/task/{supervisor}/children")).ok()?;
        let args = [
            self.binary.as_os_str().as_bytes(),
            BOOTSTRAP,
            role.argument(),
        ];
        children.split_ascii_whitespace().find_map(|text| {
            let pid = parse_pid(text)?;
            let fd = pidfd_open(pid, PidfdFlags::empty()).ok()?;
            (command_is(pid, &args) && running_as_person(pid)).then_some(RoleProcess { pid, fd })
        })
    }

    fn stop(&self) -> bool {
        self.pid().is_some_and(|pid| root_signal(pid, "-STOP"))
    }

    fn end_roles(&self) {
        let roles: Vec<_> = [Role::Daemon, Role::Holder]
            .into_iter()
            .filter_map(|role| self.process(role))
            .collect();
        for role in &roles {
            let _ = pidfd_send_signal(&role.fd, Signal::TERM);
        }
        let deadline = Instant::now() + END;
        while roles.iter().any(RoleProcess::alive) && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(20));
        }
        for role in roles.into_iter().filter(RoleProcess::alive) {
            let _ = pidfd_send_signal(&role.fd, Signal::KILL);
        }
    }
}

impl Drop for Supervisor {
    fn drop(&mut self) {
        if self.stop() {
            self.end_roles();
        }
        if let Some(pid) = self.pid()
            && root_signal(pid, "-KILL")
        {
            // Sudo mirrors a stopped command's job-control state until its caller resumes it.
            root_signal(Pid::from_child(&self.child), "-CONT");
            let _ = self.child.wait();
            return;
        }
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn root_signal(pid: Pid, signal: &str) -> bool {
    sudo("/bin/kill")
        .args([signal, "--", &pid.as_raw_pid().to_string()])
        .status()
        .is_ok_and(|status| status.success())
}

fn parse_pid(text: &str) -> Option<Pid> {
    text.parse::<i32>()
        .ok()
        .filter(|&pid| pid > 0)
        .and_then(Pid::from_raw)
}

fn command_is(pid: Pid, args: &[&[u8]]) -> bool {
    fs::read(format!("/proc/{}/cmdline", pid.as_raw_pid())).is_ok_and(|cmdline| {
        cmdline
            .strip_suffix(&[0])
            .is_some_and(|argv| argv.split(|&byte| byte == 0).eq(args.iter().copied()))
    })
}

fn running_as_person(pid: Pid) -> bool {
    let login = getuid().as_raw().to_string();
    fs::read_to_string(format!("/proc/{}/status", pid.as_raw_pid())).is_ok_and(|status| {
        status.lines().any(|line| {
            line.strip_prefix("Uid:").is_some_and(|ids| {
                ids.split_ascii_whitespace()
                    .eq(std::iter::repeat_n(login.as_str(), 4))
            })
        })
    })
}

fn sudo(program: impl AsRef<std::ffi::OsStr>) -> Command {
    let mut elevate = Command::new("/usr/bin/sudo");
    elevate.args(["-n", "--"]).arg(program);
    elevate
}

fn output(command: &mut Command, action: &str) -> Output {
    let result = command
        .output()
        .unwrap_or_else(|error| panic!("{action}: {error}"));
    assert!(
        result.status.success(),
        "{action}: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    result
}

fn stdout(output: Output) -> String {
    String::from_utf8(output.stdout).expect("the fixture command writes UTF-8")
}
