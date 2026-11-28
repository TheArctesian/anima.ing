use std::{
    ffi::OsString,
    io::{self, Read as _, Write as _},
    os::unix::{
        ffi::OsStrExt as _,
        net::{UnixListener, UnixStream},
    },
    path::Path,
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};

use super::invalid;

const WAIT: Duration = Duration::from_secs(8);

pub(super) fn with_child<T>(
    command: &mut Command,
    exchange: impl FnOnce(UnixStream) -> io::Result<T>,
) -> io::Result<(Child, T)> {
    let directory = tempfile::Builder::new()
        .prefix("anima-holder-")
        .tempdir_in("/tmp")?;
    let address = directory.path().join("bootstrap");
    let listener = UnixListener::bind(&address)?;
    listener.set_nonblocking(true)?;
    let arguments = command.get_args().map(OsString::from).collect::<Vec<_>>();
    let mut child = command.stdin(Stdio::piped()).spawn()?;
    let connected = (|| {
        child
            .stdin
            .take()
            .ok_or_else(invalid)?
            .write_all(address.as_os_str().as_bytes())?;
        let end = Instant::now() + WAIT;
        while Instant::now() < end && child.try_wait()?.is_none() {
            match listener.accept() {
                Ok((stream, _)) if signed_child(&stream, child.id(), &arguments).is_some() => {
                    return exchange(bounded(stream)?);
                }
                Ok(_) => {}
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(10));
                }
                Err(error) => return Err(error),
            }
        }
        Err(invalid())
    })();
    match connected {
        Ok(value) => Ok((child, value)),
        Err(error) => {
            let _ = child.kill();
            let _ = child.wait();
            Err(error)
        }
    }
}

pub(super) fn connect() -> io::Result<UnixStream> {
    let mut address = Vec::with_capacity(128);
    io::stdin().lock().take(128).read_to_end(&mut address)?;
    if address.len() >= 128 {
        return Err(invalid());
    }
    bounded(UnixStream::connect(Path::new(
        std::ffi::OsStr::from_bytes(&address),
    ))?)
}

fn bounded(stream: UnixStream) -> io::Result<UnixStream> {
    stream.set_read_timeout(Some(WAIT))?;
    stream.set_write_timeout(Some(WAIT))?;
    Ok(stream)
}

#[cfg(target_os = "macos")]
use super::peer::signed_child;
#[cfg(not(target_os = "macos"))]
const fn signed_child(_: &UnixStream, _: u32, _: &[OsString]) -> Option<()> {
    None
}
