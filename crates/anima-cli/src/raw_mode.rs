//! `crossterm` does the `unsafe` work, so this crate keeps `unsafe_code` forbidden (CMD-010).

use std::io;
use std::sync::Once;

#[cfg(unix)]
use crossterm::terminal;

/// Raw mode while it lives; dropping it, or a panic, restores the previous mode.
pub struct RawMode(());

static PANIC_RESTORES: Once = Once::new();

impl RawMode {
    /// # Errors
    /// The OS error when the terminal cannot change mode.
    pub fn enter() -> io::Result<Self> {
        PANIC_RESTORES.call_once(|| {
            let previous = std::panic::take_hook();
            std::panic::set_hook(Box::new(move |panic| {
                Self::restore();
                previous(panic);
            }));
        });
        #[cfg(unix)]
        terminal::enable_raw_mode()?;
        #[cfg(windows)]
        anima_windows::console::enter()?;
        Ok(Self(()))
    }

    /// For an exit that skips destructors; does nothing when raw mode is off.
    pub fn restore() {
        #[cfg(unix)]
        let _restored = terminal::disable_raw_mode();
        #[cfg(windows)]
        let _restored = anima_windows::console::restore();
    }
}

impl Drop for RawMode {
    fn drop(&mut self) {
        Self::restore();
    }
}
