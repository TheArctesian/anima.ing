//! When the device is asked for a screen of the size of the client's view (RA-004), once the view settles.

use std::time::Duration;

use ironrdp_displaycontrol::pdu::MonitorLayoutEntry;

const VIEW_SETTLES: Duration = Duration::from_millis(300);
const ASK_AGAIN: Duration = Duration::from_secs(1);
const MOST_ASKS_AGAIN: u8 = 5;

/// The screen size RDP allows nearest to a view: each side 200 to 8192 pixels, an even width.
#[must_use]
pub fn screen_for(view: (usize, usize)) -> (u16, u16) {
    let side = |side: usize| u32::try_from(side).unwrap_or(u32::MAX);
    let (width, height) = MonitorLayoutEntry::adjust_display_size(side(view.0), side(view.1));
    let side = |side: u32| u16::try_from(side).unwrap_or(u16::MAX);
    (side(width), side(height))
}

#[derive(Default)]
pub struct ViewRequests {
    waiting: Option<Waiting>,
}

struct Waiting {
    size: (u16, u16),
    ask_at: Duration,
    asks_again: u8,
}

impl ViewRequests {
    pub fn view_changed(&mut self, view: (usize, usize), now: Duration) {
        self.waiting = Some(Waiting {
            size: screen_for(view),
            ask_at: now + VIEW_SETTLES,
            asks_again: MOST_ASKS_AGAIN,
        });
    }

    /// When to call [`ViewRequests::due`] next; `None` while nothing waits.
    #[must_use]
    pub fn ask_at(&self) -> Option<Duration> {
        self.waiting.as_ref().map(|waiting| waiting.ask_at)
    }

    /// The size to ask for now; `None` before the time or when `screen` already has it.
    pub fn due(&mut self, screen: (usize, usize), now: Duration) -> Option<(u16, u16)> {
        let waiting = self
            .waiting
            .as_ref()
            .filter(|waiting| waiting.ask_at <= now)?;
        let size = waiting.size;
        if (usize::from(size.0), usize::from(size.1)) == screen {
            self.waiting = None;
            return None;
        }
        Some(size)
    }

    pub fn asked(&mut self) {
        self.waiting = None;
    }

    pub fn not_asked(&mut self, now: Duration) {
        self.waiting = self.waiting.take().and_then(|waiting| {
            Some(Waiting {
                ask_at: now + ASK_AGAIN,
                asks_again: waiting.asks_again.checked_sub(1)?,
                ..waiting
            })
        });
    }
}
