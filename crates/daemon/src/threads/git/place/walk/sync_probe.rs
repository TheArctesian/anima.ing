//! Scoped fault injection immediately before the directory sync syscall in tests.

use std::cell::RefCell;
use std::io;
use std::marker::PhantomData;
use std::os::fd::{AsFd as _, BorrowedFd, OwnedFd};
use std::rc::Rc;

type Probe = dyn FnMut(BorrowedFd<'_>) -> io::Result<()>;

thread_local! {
    static PROBE: RefCell<Option<Box<Probe>>> = RefCell::new(None);
}

pub(in crate::threads::git) struct Guard(PhantomData<Rc<()>>);

impl Drop for Guard {
    fn drop(&mut self) {
        PROBE.with_borrow_mut(|probe| *probe = None);
    }
}

fn install(probe: impl FnMut(BorrowedFd<'_>) -> io::Result<()> + 'static) -> Guard {
    PROBE.with_borrow_mut(|installed| {
        assert!(installed.is_none(), "a sync probe is already installed");
        *installed = Some(Box::new(probe));
    });
    Guard(PhantomData)
}

pub(super) fn before(fd: &OwnedFd) -> io::Result<()> {
    PROBE.with_borrow_mut(|probe| probe.as_mut().map_or(Ok(()), |call| call(fd.as_fd())))
}

pub(in crate::threads::git) fn fail_for(
    path: &std::path::Path,
    mut before_error: impl FnMut() + 'static,
) -> Guard {
    use std::os::unix::fs::MetadataExt as _;

    let metadata = path.metadata().expect("the target to sync");
    install(move |fd| {
        let opened = std::fs::File::from(fd.try_clone_to_owned()?).metadata()?;
        if opened.dev() == metadata.dev() && opened.ino() == metadata.ino() {
            before_error();
            return Err(io::Error::other("injected directory sync failure"));
        }
        Ok(())
    })
}
