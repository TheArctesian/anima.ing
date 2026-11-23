//! The descriptors that the holder got from the process that started it, which no program of the
//! holder gets (THR-040).

/// Marks each descriptor above standard error that the holder got from the process that started
/// it close-on-exec, so that no program of the holder gets it (THR-040): the three of a program
/// do not depend on what that process left open. The daemon leaves none; a shell, a test runner
/// or the step of a CI job can (run 45131440281 on GitHub's Mac passed one on). Call it first,
/// before the holder opens a descriptor of its own.
pub fn keep_inherited_from_programs() {
    let Ok(listing) = std::fs::read_dir("/dev/fd") else {
        return;
    };
    let inherited: Vec<std::os::fd::RawFd> = listing
        .filter_map(|entry| entry.ok()?.file_name().to_str()?.parse().ok())
        .filter(|fd| *fd > 2)
        .collect();
    for fd in inherited {
        // The folder of the listing itself is closed by now: its EBADF is the one expected
        // error, and a descriptor that cannot be marked cannot be closed either.
        let _ = nix::fcntl::fcntl(
            fd,
            nix::fcntl::FcntlArg::F_SETFD(nix::fcntl::FdFlag::FD_CLOEXEC),
        );
    }
}
