//! The steps of the launcher (R-331, R-330, R-347; contract K11 Sections 9 and 9.1): join the
//! cgroup of the server, set the parent-death signal as a backstop, check that the daemon is still
//! the parent, check the runtime against the signed list when one is given, hide the cgroups above
//! the server in a new cgroup namespace, and become the runtime. No shell runs, and nothing is
//! logged: a failed step writes its code ([`crate::code::Step`]) to the failure file and exits.

use std::fs::File;
use std::io::{self, Read as _};
use std::os::fd::AsRawFd as _;
use std::path::Path;

use sha2::{Digest as _, Sha256};
use updater_signature::PublicKey;

use crate::code::Step;
use crate::list;

/// The largest signature file that is read; one is a few hundred bytes.
const MAX_SIGNATURE_BYTES: u64 = 4096;

/// What one start needs: `<cgroup.procs> <daemon pid> <failure file> <program> <arguments…>`.
pub struct Order<'a> {
    pub procs: &'a str,
    pub daemon: &'a str,
    pub failure: &'a str,
    pub program: &'a str,
    pub arguments: &'a [String],
}

impl<'a> Order<'a> {
    /// The order of `arguments`, or `None` when one of the first four is missing.
    #[must_use]
    pub fn parse(arguments: &'a [String]) -> Option<Self> {
        match arguments {
            [procs, daemon, failure, program, rest @ ..] => Some(Self {
                procs,
                daemon,
                failure,
                program,
                arguments: rest,
            }),
            _ => None,
        }
    }
}

/// The signed list that the runtime must be on (the root launcher only).
pub struct Signed<'a> {
    pub list: &'a Path,
    pub signature: &'a Path,
    pub key: &'a PublicKey,
}

/// Runs the steps of `order` and becomes the runtime; it never returns. With `signed`, a runtime
/// program whose SHA-256 is not on that list is refused with [`Step::NotSigned`], and the open
/// file that was checked runs. With no list, the program runs from its path, as the launch mode
/// of the daemon always did.
///
/// The parent-death signal fires when the *thread* that started the launcher ends, not the
/// daemon process; the daemon starts servers from a thread that lives as long as it does. Call
/// this with one thread only: a new user namespace needs it.
pub fn run(order: &Order<'_>, signed: Option<&Signed<'_>>) -> ! {
    use std::os::unix::process::CommandExt as _;
    let failure = order.failure;
    let joined = Path::new(order.procs)
        .file_name()
        .is_some_and(|name| name == "cgroup.procs")
        && std::fs::write(order.procs, std::process::id().to_string()).is_ok();
    if !joined {
        fail(failure, Step::CgroupJoin);
    }
    if rustix::process::set_parent_process_death_signal(Some(rustix::process::Signal::KILL))
        .is_err()
    {
        fail(failure, Step::Backstop);
    }
    // The daemon may have ended before the signal was set: then nothing would end the runtime.
    let parent = rustix::process::getppid().map(rustix::process::Pid::as_raw_nonzero);
    if parent.is_none_or(|pid| order.daemon.parse().ok() != Some(pid.get())) {
        fail(failure, Step::ParentGone);
    }
    let checked = signed.map(|signed| {
        checked_program(order.program, signed).unwrap_or_else(|step| fail(failure, step))
    });
    if hide_cgroups().is_err() {
        fail(failure, Step::Userns);
    }
    // Duplication preserves the handshake while exec's stdio setup restores /dev/null.
    // Successful exec closes the duplicate in the kernel; runtime code cannot forge it.
    let ready = rustix::io::fcntl_dupfd_cloexec(std::io::stdin(), 3)
        .unwrap_or_else(|_| fail(failure, Step::Exec));
    if rustix::io::write(&ready, &[0]) != Ok(1) {
        fail(failure, Step::Exec);
    }
    // A checked program runs from its open file, not its path, so the file that was checked is
    // the file that runs.
    let path = checked.as_ref().map_or_else(
        || order.program.to_owned(),
        |file| format!("/proc/self/fd/{}", file.as_raw_fd()),
    );
    let _error = std::process::Command::new(path)
        .arg0(order.program)
        .args(order.arguments)
        .stdin(std::process::Stdio::null())
        .exec();
    let _ = rustix::io::write(&ready, &[1]);
    fail(failure, Step::Exec);
}

/// Whether the runtime program at `path` is a regular file whose SHA-256 is on the list of
/// `signed` for this system. The daemon checks this before it starts the launcher, which checks
/// it again on the file that it runs.
#[must_use]
pub fn is_signed(path: &str, signed: &Signed<'_>) -> bool {
    open_program(path).is_ok_and(|mut program| on_list(&mut program, signed))
}

/// The open runtime program at `path` when its SHA-256 is on the list of `signed`. The file stays
/// open across `exec`: an interpreter of a `#!` program reads it through `/proc/self/fd/<n>`
/// after the kernel has read the first line.
///
/// # Errors
/// [`Step::Exec`] when the program cannot be opened; [`Step::NotSigned`] when it is not on a
/// valid list.
fn checked_program(path: &str, signed: &Signed<'_>) -> Result<File, Step> {
    let mut program = open_program(path).map_err(|_| Step::Exec)?;
    if !on_list(&mut program, signed) {
        return Err(Step::NotSigned);
    }
    rustix::io::fcntl_setfd(&program, rustix::io::FdFlags::empty()).map_err(|_| Step::Exec)?;
    Ok(program)
}

/// Opens the runtime program: no symbolic link, no FIFO, a regular file only.
fn open_program(path: &str) -> io::Result<File> {
    use rustix::fs::{FileType, Mode, OFlags};
    let flags = OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC | OFlags::NONBLOCK;
    let fd = rustix::fs::open(path, flags, Mode::empty())?;
    if FileType::from_raw_mode(rustix::fs::fstat(&fd)?.st_mode) != FileType::RegularFile {
        return Err(io::ErrorKind::InvalidInput.into());
    }
    Ok(File::from(fd))
}

/// Whether the SHA-256 of `program` is on the list of `signed` for this system.
fn on_list(program: &mut File, signed: &Signed<'_>) -> bool {
    let Ok(text) = read_capped(signed.list, list::MAX_BYTES as u64) else {
        return false;
    };
    let Ok(signature) = read_capped(signed.signature, MAX_SIGNATURE_BYTES) else {
        return false;
    };
    let Ok(signature) = String::from_utf8(signature) else {
        return false;
    };
    let Ok(runtimes) = list::read_signed(&text, &signature, signed.key) else {
        return false;
    };
    let mut hash = Sha256::new();
    let mut chunk = vec![0_u8; 1 << 16];
    loop {
        match program.read(&mut chunk) {
            Ok(0) => break,
            Ok(read) => hash.update(&chunk[..read]),
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            Err(_) => return false,
        }
    }
    let sha256 = list::hex(&hash.finalize());
    runtimes.allows(&sha256, std::env::consts::OS, std::env::consts::ARCH)
}

/// The bytes of the file at `path`, when it is at most `cap` bytes long.
fn read_capped(path: &Path, cap: u64) -> io::Result<Vec<u8>> {
    use std::os::unix::fs::OpenOptionsExt as _;
    let file = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(rustix::fs::OFlags::NONBLOCK.bits().cast_signed())
        .open(path)?;
    let mut bytes = Vec::new();
    file.take(cap + 1).read_to_end(&mut bytes)?;
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > cap {
        return Err(io::ErrorKind::InvalidData.into());
    }
    Ok(bytes)
}

/// Moves the launcher into a new cgroup namespace whose root is the cgroup of its server
/// (R-330): the runtime then sees `/` as its cgroup, and the kernel refuses a move of a process
/// to a cgroup outside that root, such as the cgroup of the daemon. A user of the system may make
/// a cgroup namespace only inside a user namespace of its own, so both are made in one call; the
/// user namespace maps the user and the group of the daemon to themselves.
///
/// # Errors
/// An I/O error when the system does not permit the namespaces; the launcher then refuses the
/// start, with no fallback.
fn hide_cgroups() -> io::Result<()> {
    use rustix::thread::UnshareFlags;
    let uid = rustix::process::getuid().as_raw();
    let gid = rustix::process::getgid().as_raw();
    #[expect(
        deprecated,
        reason = "unshare_unsafe needs `unsafe`, which the workspace forbids; its hazard is \
                  CLONE_FILES and CLONE_FS, which this call does not pass"
    )]
    rustix::thread::unshare(UnshareFlags::NEWUSER | UnshareFlags::NEWCGROUP)?;
    std::fs::write("/proc/self/setgroups", "deny")?;
    std::fs::write("/proc/self/uid_map", format!("{uid} {uid} 1"))?;
    std::fs::write("/proc/self/gid_map", format!("{gid} {gid} 1"))?;
    Ok(())
}

/// Writes the exit code of the failed step to the file `failure`, then exits with it.
fn fail(failure: &str, step: Step) -> ! {
    use std::io::Write as _;
    use std::os::unix::fs::OpenOptionsExt as _;
    let code = step.code();
    // A new file only (the daemon removes it before each start), no link, no wait on a FIFO.
    let file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .custom_flags(
            (rustix::fs::OFlags::NOFOLLOW | rustix::fs::OFlags::NONBLOCK)
                .bits()
                .cast_signed(),
        )
        .open(failure);
    if let Ok(mut file) = file {
        let _ = file.write_all(code.to_string().as_bytes());
    }
    std::process::exit(code);
}
