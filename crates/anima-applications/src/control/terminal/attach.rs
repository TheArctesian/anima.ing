//! Relay bytes between the local terminal and the one attached Docker exec.
use std::{
    fs::{File, OpenOptions},
    io::{self, BufReader, Read, Write},
    os::unix::{fs::OpenOptionsExt as _, net::UnixStream},
};

use nix::sys::{
    signal::{SigSet, Signal},
    signalfd::{SfdFlags, SignalFd},
};
use rustix::{
    event::{PollFd, PollFlags, poll},
    io::Errno,
    termios::{OptionalActions, Termios, tcgetattr, tcgetwinsize, tcsetattr},
};

use super::{Error, Target, engine::Engine};

pub(super) struct Terminal {
    tty: File,
    original: Termios,
    signals: SignalFd,
}

impl Terminal {
    pub(super) fn open() -> Result<Self, Error> {
        let tty = OpenOptions::new()
            .read(true)
            .write(true)
            .custom_flags(nix::libc::O_NONBLOCK | nix::libc::O_CLOEXEC)
            .open("/dev/tty")?;
        let original = tcgetattr(&tty)?;
        let mask: SigSet = [
            Signal::SIGHUP,
            Signal::SIGTERM,
            Signal::SIGINT,
            Signal::SIGQUIT,
            Signal::SIGWINCH,
        ]
        .into_iter()
        .collect();
        mask.thread_block().map_err(io::Error::from)?;
        let signals = SignalFd::with_flags(&mask, SfdFlags::SFD_NONBLOCK | SfdFlags::SFD_CLOEXEC)
            .map_err(io::Error::from)?;
        Ok(Self {
            tty,
            original,
            signals,
        })
    }

    pub(super) fn dimensions(&self) -> Result<(u16, u16), Error> {
        let size = tcgetwinsize(&self.tty)?;
        Ok((size.ws_row.max(1), size.ws_col.max(1)))
    }

    pub(super) fn header(&mut self, target: &Target) -> Result<(), Error> {
        writeln!(
            self.tty,
            "Application {} · {} · {}",
            target.application, target.service, target.user
        )?;
        Ok(())
    }

    pub(super) fn relay(
        &mut self,
        mut socket: BufReader<UnixStream>,
        (engine, id): (&Engine<'_>, &str),
    ) -> Result<(), Error> {
        if self.closing((engine, id))? {
            return Ok(());
        }
        let mut raw = self.original.clone();
        raw.make_raw();
        tcsetattr(&self.tty, OptionalActions::Flush, &raw)?;
        socket.get_mut().write_all(b"open\n")?;
        socket.get_ref().set_nonblocking(true)?;
        let mut input = Buffer::new();
        let mut output = Buffer::new();
        loop {
            if output.empty() && !socket.buffer().is_empty() {
                output.read(&mut socket)?;
            }
            let (tty, peer, signals) = wait(
                &self.tty,
                socket.get_ref(),
                &self.signals,
                (&input, &output),
            )?;
            if (signals && self.closing((engine, id))?) || disconnected(tty) {
                return Ok(());
            }
            if tty.contains(PollFlags::OUT) {
                output.flush(&mut self.tty)?;
            }
            if peer.contains(PollFlags::HUP) {
                input.start = input.end;
            } else if peer.contains(PollFlags::OUT) {
                input.flush(socket.get_mut())?;
            }
            if !peer.contains(PollFlags::HUP)
                && tty.contains(PollFlags::IN)
                && input.read(&mut self.tty)?
            {
                return Ok(());
            }
            if output.empty()
                && peer.intersects(PollFlags::IN | PollFlags::HUP)
                && output.read(&mut socket)?
            {
                return Ok(());
            }
            if peer.intersects(PollFlags::ERR | PollFlags::NVAL) {
                return Err(Error::Terminal(io::ErrorKind::ConnectionAborted));
            }
        }
    }

    fn closing(&self, (engine, id): (&Engine<'_>, &str)) -> Result<bool, Error> {
        let mut resized = false;
        while let Some(signal) = self.signals.read_signal().map_err(io::Error::from)? {
            if signal.ssi_signo != nix::libc::SIGWINCH.unsigned_abs() {
                return Ok(true);
            }
            resized = true;
        }
        if resized {
            engine.resize(id, self.dimensions()?)?;
        }
        Ok(false)
    }
}

impl Drop for Terminal {
    fn drop(&mut self) {
        let _ = tcsetattr(&self.tty, OptionalActions::Flush, &self.original);
    }
}

fn wait(
    tty: &File,
    peer: &UnixStream,
    signals: &SignalFd,
    (input, output): (&Buffer, &Buffer),
) -> Result<(PollFlags, PollFlags, bool), Error> {
    let mut fds = [
        PollFd::new(tty, interests(input.empty(), !output.empty())),
        PollFd::new(signals, PollFlags::IN),
        PollFd::new(peer, interests(output.empty(), !input.empty())),
    ];
    let used = if output.empty() || !input.empty() {
        3
    } else {
        2
    };
    loop {
        match poll(&mut fds[..used], None) {
            Err(Errno::INTR) => (),
            Err(error) => return Err(error.into()),
            Ok(_) => {
                return Ok((
                    fds[0].revents(),
                    fds[2].revents(),
                    !fds[1].revents().is_empty(),
                ));
            }
        }
    }
}

fn interests(read: bool, write: bool) -> PollFlags {
    let mut flags = PollFlags::empty();
    flags.set(PollFlags::IN, read);
    flags.set(PollFlags::OUT, write);
    flags
}

fn disconnected(flags: PollFlags) -> bool {
    flags.intersects(PollFlags::ERR | PollFlags::NVAL | PollFlags::HUP)
}

struct Buffer {
    bytes: [u8; 8192],
    start: usize,
    end: usize,
}

impl Buffer {
    const fn new() -> Self {
        Self {
            bytes: [0; 8192],
            start: 0,
            end: 0,
        }
    }
    const fn empty(&self) -> bool {
        self.start == self.end
    }

    fn read(&mut self, input: &mut impl Read) -> io::Result<bool> {
        match input.read(&mut self.bytes) {
            Ok(0) => Ok(true),
            Ok(count) => {
                self.start = 0;
                self.end = count;
                Ok(false)
            }
            Err(error) if interrupted(&error) => Ok(false),
            Err(error) => Err(error),
        }
    }

    fn flush(&mut self, output: &mut impl Write) -> io::Result<()> {
        match output.write(&self.bytes[self.start..self.end]) {
            Ok(0) if !self.empty() => Err(io::ErrorKind::WriteZero.into()),
            Ok(count) => {
                self.start += count;
                Ok(())
            }
            Err(error) if interrupted(&error) => Ok(()),
            Err(error) => Err(error),
        }
    }
}

fn interrupted(error: &io::Error) -> bool {
    matches!(
        error.kind(),
        io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
    )
}
