//! The fixed Linux role executable transitions from its root supervisor to a login account.

use std::fs::File;
use std::io::{self, Read as _, Seek as _, SeekFrom};
use std::os::fd::OwnedFd;
use std::path::{Component, Path};

use rustix::fs::{self, FileType, Mode, OFlags, Stat};
use rustix::process::{
    self, DumpableBehavior, Gid, Resource, Rlimit, Uid, dumpable_behavior, set_dumpable_behavior,
};
use rustix::thread::{
    self, CapabilitySet, CapabilitySets, set_thread_groups, set_thread_res_gid, set_thread_res_uid,
};

const CORE_LIMIT: Rlimit = Rlimit {
    current: Some(0),
    maximum: Some(0),
};
const NO_CAPABILITIES: CapabilitySets = CapabilitySets {
    effective: CapabilitySet::empty(),
    permitted: CapabilitySet::empty(),
    inheritable: CapabilitySet::empty(),
};

fn refused(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::PermissionDenied, message)
}

/// Check the supervisor's fixed absolute program path before launch. A root administrator
/// controls the installation and its updates. Each path component is opened without following
/// symlinks, using the preceding directory's descriptor.
pub(super) fn trusted_program(path: &Path) -> io::Result<()> {
    let (Some(parent), Some(name)) = (path.parent(), path.file_name()) else {
        return Err(refused("missing role executable path"));
    };
    let directory = open_trusted_directory(parent)?;
    let flags =
        OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::NOCTTY | OFlags::CLOEXEC;
    let file = fs::openat(&directory, name, flags, Mode::empty())?;
    check_program(File::from(file))
}

/// Validate the root-controlled directory containing the supervisor's configuration.
pub(super) fn trusted_directory(path: &Path) -> io::Result<()> {
    open_trusted_directory(path).map(drop)
}

fn open_trusted_directory(path: &Path) -> io::Result<OwnedFd> {
    let mut components = path.components();
    if components.next() != Some(Component::RootDir) {
        return Err(refused("expected an absolute role path"));
    }
    let directories = OFlags::PATH | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC;
    let mut directory = fs::open("/", directories, Mode::empty())?;
    check_owner(&fs::fstat(&directory)?)?;
    for component in components {
        let Component::Normal(name) = component else {
            return Err(refused("invalid role path component"));
        };
        directory = fs::openat(&directory, name, directories, Mode::empty())?;
        check_owner(&fs::fstat(&directory)?)?;
    }
    Ok(directory)
}

fn check_owner(stat: &Stat) -> io::Result<()> {
    let mode = Mode::from_raw_mode(stat.st_mode);
    if stat.st_uid != 0 || mode.intersects(Mode::WGRP | Mode::WOTH) {
        return Err(refused(
            "role executable path permits non-root modification",
        ));
    }
    Ok(())
}

fn check_program(mut file: File) -> io::Result<()> {
    let stat = fs::fstat(&file)?;
    check_owner(&stat)?;
    let mode = Mode::from_raw_mode(stat.st_mode);
    if FileType::from_raw_mode(stat.st_mode) != FileType::RegularFile
        || !mode.contains(Mode::XUSR)
        || mode.intersects(Mode::SUID | Mode::SGID)
    {
        return Err(refused(
            "role executable must be an ordinary executable file",
        ));
    }
    match fs::fgetxattr_query(&file, "security.capability") {
        Err(rustix::io::Errno::NODATA) => {}
        Err(error) => return Err(error.into()),
        Ok(_) => return Err(refused("role executable has file capabilities")),
    }
    let mut magic = [0; 4];
    file.read_exact(&mut magic)?;
    if magic != *b"\x7fELF" {
        return Err(refused("role executable must be an ELF program"));
    }
    Ok(())
}

/// Run in the root bootstrap before spawning any threads. IDs come from the supervisor's
/// configured login account. On error the caller must terminate the process immediately.
pub(super) fn drop_privileges(uid: u32, gid: u32, supplementary: &[u32]) -> io::Result<()> {
    if [uid, gid]
        .into_iter()
        .chain(supplementary.iter().copied())
        .any(|id| id == 0 || id == u32::MAX)
    {
        return Err(refused("invalid role user or group ID"));
    }
    if !process::getuid().is_root() || !process::geteuid().is_root() {
        return Err(refused("role bootstrap requires root"));
    }
    let mut sysctl = String::new();
    open_proc("/proc/sys/fs/suid_dumpable")?.read_to_string(&mut sysctl)?;
    if sysctl.trim() != "0" {
        return Err(refused("role bootstrap requires fs.suid_dumpable=0"));
    }
    set_dumpable_behavior(DumpableBehavior::NotDumpable)?;
    process::setrlimit(Resource::Core, CORE_LIMIT)?;
    let mut proc_status = open_proc("/proc/thread-self/status")?;
    let before = read_status(&mut proc_status)?;
    check_bootstrap(&before)?;
    let mut groups = supplementary.to_vec();
    groups.sort_unstable();
    let target_groups: Vec<_> = groups.into_iter().map(Gid::from_raw).collect();

    thread::set_no_new_privs(true)?;
    thread::clear_ambient_capability_set()?;
    clear_bounding_set(capability_mask(&before, "CapBnd")?)?;
    set_thread_groups(&target_groups)?;
    let gid = Gid::from_raw(gid);
    set_thread_res_gid(gid, gid, gid)?;
    let uid = Uid::from_raw(uid);
    set_thread_res_uid(uid, uid, uid)?;

    thread::set_capabilities(None, NO_CAPABILITIES)?;
    verify_drop(&mut proc_status, uid, gid, &target_groups)
}

fn open_proc(path: &str) -> io::Result<File> {
    let file = File::open(path)?;
    if fs::fstatfs(&file)?.f_type != fs::PROC_SUPER_MAGIC {
        return Err(refused("role bootstrap requires procfs"));
    }
    Ok(file)
}

fn read_status(file: &mut File) -> io::Result<String> {
    let mut status = String::new();
    file.seek(SeekFrom::Start(0))?;
    file.read_to_string(&mut status)?;
    Ok(status)
}

fn status_field<'a>(status: &'a str, key: &str) -> io::Result<&'a str> {
    status
        .lines()
        .filter_map(|line| line.split_once(':'))
        .find_map(|(field, value)| (field == key).then(|| value.trim()))
        .ok_or_else(|| refused("missing procfs role status field"))
}

fn status_number(status: &str, key: &str) -> io::Result<u32> {
    status_field(status, key)?
        .parse()
        .map_err(|_| refused("invalid procfs role status number"))
}

fn ids_match(status: &str, key: &str, expected: u32) -> io::Result<bool> {
    let mut ids = status_field(status, key)?.split_ascii_whitespace();
    Ok(
        (0..4).all(|_| ids.next().and_then(|id| id.parse().ok()) == Some(expected))
            && ids.next().is_none(),
    )
}

fn check_bootstrap(status: &str) -> io::Result<()> {
    let pid = std::process::id();
    if status_number(status, "Pid")? != pid
        || status_number(status, "Tgid")? != pid
        || status_number(status, "Threads")? != 1
        || status_number(status, "TracerPid")? != 0
        || !ids_match(status, "Uid", 0)?
    {
        return Err(refused(
            "role bootstrap requires an untraced single root thread",
        ));
    }
    Ok(())
}

fn capability_mask(status: &str, key: &str) -> io::Result<u64> {
    u64::from_str_radix(status_field(status, key)?, 16)
        .map_err(|_| refused("invalid procfs role capability mask"))
}

fn clear_bounding_set(mask: u64) -> io::Result<()> {
    for index in 0..u64::BITS {
        let bit = 1_u64 << index;
        if mask & bit != 0 {
            thread::remove_capability_from_bounding_set(CapabilitySet::from_bits_retain(bit))?;
        }
    }
    Ok(())
}

fn verify_drop(file: &mut File, uid: Uid, gid: Gid, groups: &[Gid]) -> io::Result<()> {
    let status = read_status(file)?;
    if !ids_match(&status, "Uid", uid.as_raw())?
        || !ids_match(&status, "Gid", gid.as_raw())?
        || process::getgroups()? != groups
        || dumpable_behavior()? != DumpableBehavior::NotDumpable
        || thread::capabilities(None)? != NO_CAPABILITIES
        || capability_mask(&status, "CapAmb")? != 0
        || capability_mask(&status, "CapBnd")? != 0
        || !thread::no_new_privs()?
        || process::getrlimit(Resource::Core) != CORE_LIMIT
    {
        return Err(refused(
            "role bootstrap could not verify the privilege drop",
        ));
    }
    Ok(())
}
