//! The picture a session sends, as frames no faster than the client draws them.

use guac_protocol::Instruction;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::time::Instant;

use super::hub::Hub;
use super::opening::{Allowed, refuse};
use super::{Ended, status};
use crate::frames::{self, Formats, Next, Pace, Pacing};
use crate::screen::{Area, BadSize, Screen};
use crate::view::View;

pub(super) struct Picture {
    pub(super) screen: Screen,
    view: View,
    pacing: Pacing,
    formats: Formats,
    started: Instant,
    after_frame: Option<Instruction>,
}

impl Picture {
    pub(super) fn new(
        size: (usize, usize),
        allowed: &Allowed,
        pace: Pace,
    ) -> Result<Self, BadSize> {
        let screen = Screen::new(size.0, size.1)?;
        Ok(Self {
            view: View::new(&screen, allowed.view),
            screen,
            pacing: Pacing::new(pace),
            formats: allowed.formats,
            started: Instant::now(),
            after_frame: None,
        })
    }

    /// A frame that the client holds back goes after the client's next `sync` instruction.
    pub(super) fn due(&self) -> Option<Instant> {
        match self.pacing.next(self.started.elapsed()) {
            Next::At(time) if self.view.has_changes(&self.screen) => Some(self.started + time),
            _ => None,
        }
    }

    pub(super) fn drawn(&mut self, timestamp: &str) {
        self.pacing.answered(timestamp);
    }

    pub(super) fn set_view(&mut self, view: (usize, usize)) {
        self.view.set_client(view, &mut self.screen);
    }

    pub(super) fn show_display(&mut self, area: Option<Area>, layout: Instruction) {
        self.view.set_region(area, &mut self.screen);
        self.screen.forget_what_was_told();
        self.after_frame = Some(layout);
    }

    pub(super) fn switching_display(&self) -> bool {
        self.after_frame.is_some()
    }

    pub(super) fn to_device(&self, at: (usize, usize)) -> (u16, u16) {
        let (x, y) = self.view.to_device(at, &self.screen);
        let on_wire = |at: usize| u16::try_from(at).unwrap_or(u16::MAX);
        (on_wire(x), on_wire(y))
    }

    pub(super) async fn send_frame<S>(&mut self, hub: &mut Hub<S>) -> Result<(), Ended>
    where
        S: AsyncRead + AsyncWrite + Send + 'static,
    {
        if !self.view.has_changes(&self.screen) {
            return Ok(());
        }
        let Some(timestamp) = self.pacing.send(self.started.elapsed()) else {
            return Ok(());
        };
        let (shown, changes) = self.view.take_changes(&mut self.screen);
        let Ok(mut frame) = frames::frame(shown, &changes, self.formats, timestamp) else {
            return Err(refuse(hub, status::SERVER_ERROR, Ended::Fault).await);
        };
        frame.extend(self.after_frame.take());
        hub.send(&frame).await.map_err(|_| Ended::ByHub)
    }
}
