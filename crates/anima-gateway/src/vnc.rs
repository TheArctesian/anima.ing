//! The VNC client (RFB 3.8, RFC 6143).
//!
//! The device is not trusted: each length it sends is checked against a limit before use, and no bytes from a device can make this code panic.

mod auth;
mod drawing;
mod layout;
mod messages;
pub mod requests;
mod sign_in;
mod zrle;

use crate::screen::Screen;
pub use layout::Monitor;
use messages::{Body, Start};
pub use sign_in::{Account, Method, Offered, SignInError, offered, sign_in};

/// One update may send this many times the screen's pixels, so one message cannot keep the gateway busy.
const MOST_SCREENS_IN_AN_UPDATE: usize = 2;
/// Above this many bytes of room, the unread buffer is given back once it is nearly empty.
const LARGE_BUFFER: usize = 1 << 20;

/// Why a device's bytes cannot be read further. The session ends.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum Malformed {
    #[error("a message type the gateway did not ask for")]
    UnknownMessage,
    #[error("an encoding the gateway did not ask for")]
    UnknownEncoding,
    #[error("a screen size outside the limits")]
    BadSize,
    #[error("more pixels in one update than the limit")]
    TooManyPixels,
    #[error("a rectangle outside the screen")]
    OutsideScreen,
    #[error("a pointer shape larger than the screen")]
    LargePointer,
    #[error("more bytes for a rectangle than its pixels need")]
    TooManyBytes,
    #[error("a ZRLE tile that does not decode")]
    BadTile,
    #[error("zlib data that does not inflate")]
    BadZlib,
    #[error("pixel data of the wrong length")]
    BadPixels,
}

/// What a device did, after the screen copy has taken it in.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Event {
    Updated,
    /// The device's screen has a new size and is black: the next request asks for all of it.
    Resized,
    Displays {
        monitors: Vec<Monitor>,
        resized: bool,
    },
    Clipboard(String),
    Bell,
}

struct Update {
    rects_left: usize,
    pixels_left: usize,
    resized: bool,
}

impl Update {
    fn account_for(&mut self, rect: &messages::Rect<'_>) -> Result<(), Malformed> {
        let pixels = rect.area.width * rect.area.height;
        if matches!(rect.body, Body::Layout(_) | Body::NewSize) {
            // The rest of the limit holds the new screen at least once, and at most its own limit.
            if std::mem::replace(&mut self.resized, true) {
                return Err(Malformed::BadSize);
            }
            self.pixels_left = self
                .pixels_left
                .min(pixels * MOST_SCREENS_IN_AN_UPDATE)
                .max(pixels);
        } else {
            self.pixels_left = self
                .pixels_left
                .checked_sub(pixels)
                .ok_or(Malformed::TooManyPixels)?;
        }
        Ok(())
    }
}

#[derive(Default)]
pub struct Device {
    /// At most one rectangle or clipboard text, and the read that completed it.
    unread: Vec<u8>,
    /// Bytes of an over-long clipboard text still to drop.
    skip: usize,
    update: Option<Update>,
    inflaters: drawing::Inflaters,
}

impl Device {
    /// Takes `bytes` as they arrive from the device and draws each rectangle into `screen`
    /// as soon as it is whole.
    ///
    /// # Errors
    /// [`Malformed`]: the stream cannot be read further and the session ends.
    pub fn receive(&mut self, bytes: &[u8], screen: &mut Screen) -> Result<Vec<Event>, Malformed> {
        let dropped = self.skip.min(bytes.len());
        self.skip -= dropped;
        self.unread.extend_from_slice(&bytes[dropped..]);
        let mut events = Vec::new();
        let mut read = 0;
        while let Some(length) = self.read_one(read, screen, &mut events)? {
            read += length;
        }
        self.unread.drain(..read);
        // The room one large rectangle needed is not kept for the rest of the session.
        if self.unread.capacity() > LARGE_BUFFER && self.unread.len() < LARGE_BUFFER / 16 {
            self.unread.shrink_to(LARGE_BUFFER / 16);
        }
        Ok(events)
    }

    fn read_one(
        &mut self,
        from: usize,
        screen: &mut Screen,
        events: &mut Vec<Event>,
    ) -> Result<Option<usize>, Malformed> {
        let unread = &self.unread[from..];
        let Some(update) = &mut self.update else {
            let Some((start, mut length)) = messages::start(unread)? else {
                return Ok(None);
            };
            match start {
                Start::Update { rects: 0 } => events.push(Event::Updated),
                Start::Update { rects } => {
                    let pixels_left = screen.width() * screen.height() * MOST_SCREENS_IN_AN_UPDATE;
                    self.update = Some(Update {
                        rects_left: rects,
                        pixels_left,
                        resized: false,
                    });
                }
                Start::Bell => events.push(Event::Bell),
                // Latin-1: each byte is the character with that number.
                Start::CutText(latin1) => {
                    events.push(Event::Clipboard(
                        latin1.iter().map(|byte| char::from(*byte)).collect(),
                    ));
                }
                Start::LongCutText { skip } => {
                    let here = skip.min(unread.len() - length);
                    length += here;
                    self.skip = skip - here;
                }
            }
            return Ok(Some(length));
        };
        let size = (screen.width(), screen.height());
        let Some((rect, length)) = messages::rect(unread, size)? else {
            return Ok(None);
        };
        update.account_for(&rect)?;
        let layout = match &rect.body {
            Body::Layout(monitors) => Some(monitors.clone()),
            _ => None,
        };
        let resized = self.inflaters.draw(&[rect], screen)?;
        if resized && layout.is_none() {
            events.push(Event::Resized);
        }
        if let Some(monitors) = layout {
            events.push(Event::Displays {
                monitors,
                resized: size != (screen.width(), screen.height()),
            });
        }
        update.rects_left -= 1;
        if update.rects_left == 0 {
            self.update = None;
            events.push(Event::Updated);
        }
        Ok(Some(length))
    }
}
