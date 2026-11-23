//! Echo stays off from before the first question until after the last answer (CMD-010).

use std::fs::File;
use std::io::{self, Read as _, Write};
use std::sync::Once;
use std::sync::atomic::{AtomicBool, Ordering};

#[cfg(unix)]
use tokio::signal::unix::{SignalKind, signal};
use zeroize::Zeroizing;

use super::line::{Ending, Key, Keys, Line};
use super::{MAX_ANSWER, prompt_text, terminal_failure};
use crate::failure::Failure;
use crate::raw_mode::RawMode;

static ON_SCREEN: AtomicBool = AtomicBool::new(false);
static SIGNALS: Once = Once::new();

pub struct Hidden {
    keys: Keys,
    raw: Option<RawMode>,
}

impl Hidden {
    pub(super) fn start() -> io::Result<Self> {
        watch_signals();
        let raw = RawMode::enter()?;
        ON_SCREEN.store(true, Ordering::SeqCst);
        Ok(Self {
            keys: Keys::default(),
            raw: Some(raw),
        })
    }

    /// # Errors
    /// [`Failure::Cancelled`] for Ctrl-C; [`Failure::Input`] for a long answer or a failed terminal.
    pub fn ask(&mut self, what: &str) -> Result<String, Failure> {
        let answer = self.line(&prompt_text(what), true)?;
        Ok(answer.trim().to_owned())
    }

    /// Shows nothing as the secret is typed.
    ///
    /// # Errors
    /// As [`Hidden::ask`].
    pub fn ask_secret(&mut self, what: &str) -> Result<Zeroizing<String>, Failure> {
        self.line(&prompt_text(what), false)
    }

    fn line(&mut self, prompt: &str, shown: bool) -> Result<Zeroizing<String>, Failure> {
        let mut screen = screen();
        let wrote = write!(screen, "{prompt}").and_then(|()| screen.flush());
        wrote.map_err(|error| terminal_failure(&error))?;
        let mut line = Line::new(MAX_ANSWER);
        let ending = self.read(&mut line, &mut screen, shown.then_some(prompt));
        let ended = write!(screen, "\r\n").and_then(|()| screen.flush());
        match ending.map_err(|error| terminal_failure(&error))? {
            Ending::Answered => ended
                .map(|()| line.into_text())
                .map_err(|error| terminal_failure(&error)),
            Ending::Interrupted => Err(Failure::Cancelled),
            Ending::TooLong => Err(Failure::Input {
                what: "the answer".to_owned(),
                reason: format!("it is longer than {MAX_ANSWER} bytes"),
            }),
            Ending::Ended => Err(Failure::Input {
                what: "the terminal".to_owned(),
                reason: "the input ended before an answer".to_owned(),
            }),
        }
    }

    fn read(
        &mut self,
        line: &mut Line,
        screen: &mut impl Write,
        shown: Option<&str>,
    ) -> io::Result<Ending> {
        let mut stdin = io::stdin().lock();
        let mut byte = [0_u8; 1];
        loop {
            match stdin.read(&mut byte) {
                Ok(0) => return Ok(Ending::Ended),
                Ok(_) => {}
                Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                Err(error) => return Err(error),
            }
            let Some(key) = self.keys.push(byte[0]) else {
                continue;
            };
            let before = line.text().len();
            if let Some(ending) = line.apply(key) {
                return Ok(ending);
            }
            let Some(prompt) = shown else {
                continue;
            };
            let added = match key {
                Key::Char(character) if line.text().len() > before => Some(character),
                Key::Char(_) => continue,
                _ => None,
            };
            show(screen, prompt, line.text(), added)?;
        }
    }
}

impl Drop for Hidden {
    fn drop(&mut self) {
        drop(self.raw.take());
        ON_SCREEN.store(false, Ordering::SeqCst);
    }
}

/// The controlling terminal, so that no question or shown answer lands in a redirected standard error.
fn screen() -> Box<dyn Write> {
    match File::options()
        .write(true)
        .open(if cfg!(windows) { "CONOUT$" } else { "/dev/tty" })
    {
        Ok(terminal) => Box::new(terminal),
        Err(_) => Box::new(io::stderr()),
    }
}

fn show(screen: &mut impl Write, prompt: &str, text: &str, added: Option<char>) -> io::Result<()> {
    match added {
        Some(character) => write!(screen, "{character}")?,
        None => write!(screen, "\r\x1b[K{prompt}{text}")?,
    }
    screen.flush()
}

/// Exits with 128 plus the signal's number, as the default action does. The signals are
/// watched on a thread of their own because a question blocks the runtime's only thread.
fn watch_signals() {
    SIGNALS.call_once(|| {
        let (ready, watched) = std::sync::mpsc::sync_channel::<()>(1);
        let spawned = std::thread::Builder::new()
            .name("anima-signals".to_owned())
            .spawn(move || {
                let Ok(runtime) = tokio::runtime::Builder::new_current_thread()
                    .enable_io()
                    .build()
                else {
                    return;
                };
                let number = runtime.block_on(first_signal(ready));
                RawMode::restore();
                if ON_SCREEN.load(Ordering::SeqCst) {
                    eprint!("\r\n");
                }
                std::process::exit(128 + number);
            });
        if spawned.is_ok() {
            // An error means the signals could not be watched; the question goes on without.
            let _watched = watched.recv();
        }
    });
}

#[cfg(unix)]
async fn first_signal(ready: std::sync::mpsc::SyncSender<()>) -> i32 {
    let kinds = (
        signal(SignalKind::interrupt()),
        signal(SignalKind::terminate()),
        signal(SignalKind::hangup()),
    );
    let (Ok(mut interrupt), Ok(mut terminate), Ok(mut hangup)) = kinds else {
        drop(ready);
        return std::future::pending().await;
    };
    let _told = ready.send(());
    tokio::select! {
        _ = interrupt.recv() => 2,
        _ = terminate.recv() => 15,
        _ = hangup.recv() => 1,
    }
}

#[cfg(windows)]
async fn first_signal(ready: std::sync::mpsc::SyncSender<()>) -> i32 {
    let (Ok(mut interrupt), Ok(mut close), Ok(mut shutdown)) = (
        tokio::signal::windows::ctrl_c(),
        tokio::signal::windows::ctrl_close(),
        tokio::signal::windows::ctrl_shutdown(),
    ) else {
        drop(ready);
        return std::future::pending().await;
    };
    let _told = ready.send(());
    tokio::select! { _ = interrupt.recv() => 2, _ = close.recv() => 15, _ = shutdown.recv() => 15 }
}
