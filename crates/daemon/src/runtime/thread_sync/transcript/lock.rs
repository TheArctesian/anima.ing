use crate::config::home::Home;

pub(super) fn journal_lock(home: &Home) -> Option<std::fs::File> {
    use rustix::fs::{FlockOperation, Mode, OFlags, flock, open};
    let fd = open(
        home.dir().join("thread-sync.lock"),
        OFlags::CREATE | OFlags::WRONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::RUSR | Mode::WUSR,
    )
    .ok()?;
    flock(&fd, FlockOperation::NonBlockingLockExclusive).ok()?;
    Some(std::fs::File::from(fd))
}
