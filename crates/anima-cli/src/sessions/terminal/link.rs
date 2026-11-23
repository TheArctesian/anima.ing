//! The running terminal tunnel (K4 Section 8.4).
//! One `select!` over cancel-safe branches; input waits in a bounded queue, so a slow hub slows
//! the reader of standard input, never a growing buffer.

use std::io::Write;
use std::pin::Pin;

use tokio::sync::{mpsc, watch};
use tokio::time::{Instant, sleep_until};

use super::codec::{self, CellSize, HubMessage, Sizing};
use super::ending::{EndReason, Ending, Stop};
use crate::hub::tunnel::{Frame, Socket, TunnelError};
use crate::sessions::pace::{Due, Pace};

pub struct Streams<'a> {
    pub keys: mpsc::Receiver<Vec<u8>>,
    pub sizes: watch::Receiver<CellSize>,
    /// Where the device's output goes, unchanged (it is the operator's terminal by design).
    pub output: &'a mut (dyn Write + Send),
    pub stop: Pin<Box<dyn Future<Output = ()> + Send + 'a>>,
}

enum Turn {
    Frame(Result<Frame, TunnelError>),
    Keys(Option<Vec<u8>>),
    Size(bool),
    Due,
    Stop,
}

struct Link<'s> {
    socket: &'s mut Socket,
    pace: Pace,
    sizing: Sizing,
    said: Option<EndReason>,
    keys_open: bool,
    sizes_open: bool,
}

pub async fn relay(socket: &mut Socket, mut streams: Streams<'_>) -> Ending {
    let mut link = Link {
        socket,
        pace: Pace::new(Instant::now()),
        sizing: Sizing::default(),
        said: None,
        keys_open: true,
        sizes_open: true,
    };
    let first = *streams.sizes.borrow_and_update();
    if let Err((stop, close)) = link.size(first).await {
        return link.ending(stop, close);
    }
    loop {
        let turn = tokio::select! {
            frame = link.socket.recv() => Turn::Frame(frame),
            keys = streams.keys.recv(), if link.keys_open => Turn::Keys(keys),
            changed = streams.sizes.changed(), if link.sizes_open => Turn::Size(changed.is_ok()),
            () = sleep_until(link.pace.next()) => Turn::Due,
            () = &mut streams.stop => Turn::Stop,
        };
        let step = match turn {
            Turn::Frame(frame) => link.on_frame(frame, streams.output),
            Turn::Keys(Some(bytes)) => link.input(&bytes).await,
            Turn::Keys(None) => {
                link.keys_open = false;
                Ok(())
            }
            Turn::Size(true) => {
                let size = *streams.sizes.borrow_and_update();
                link.size(size).await
            }
            Turn::Size(false) => {
                link.sizes_open = false;
                Ok(())
            }
            Turn::Due => link.on_due().await,
            Turn::Stop => Err((Stop::Left, None)),
        };
        if let Err((stop, close)) = step {
            return link.ending(stop, close);
        }
    }
}

type Step = Result<(), (Stop, Option<u16>)>;

impl Link<'_> {
    const fn ending(&self, stop: Stop, close: Option<u16>) -> Ending {
        Ending {
            stop,
            said: self.said,
            close,
        }
    }

    fn on_frame(&mut self, frame: Result<Frame, TunnelError>, output: &mut dyn Write) -> Step {
        self.pace.heard(Instant::now());
        match frame {
            Ok(Frame::Binary(bytes)) => output
                .write_all(&bytes)
                .and_then(|()| output.flush())
                .map_err(|_| (Stop::Left, None)),
            Ok(Frame::Text(text)) => {
                if let Some(HubMessage::End(reason)) = codec::decode(&text) {
                    self.said = Some(reason);
                }
                Ok(())
            }
            Ok(Frame::Alive) => Ok(()),
            Ok(Frame::Closed(code)) => Err((Stop::Closed, code)),
            Err(_) => Err((Stop::Lost, None)),
        }
    }

    async fn input(&mut self, bytes: &[u8]) -> Step {
        for frame in codec::input_frames(bytes) {
            self.socket
                .send_binary(frame)
                .await
                .map_err(|_| (Stop::Lost, None))?;
        }
        self.pace.sent(Instant::now());
        Ok(())
    }

    async fn size(&mut self, size: CellSize) -> Step {
        match self.sizing.update(size) {
            Some(message) => self.text(&message).await,
            None => Ok(()),
        }
    }

    async fn on_due(&mut self) -> Step {
        match self.pace.due(Instant::now()) {
            Some(Due::Ping) => self.text(codec::PING).await,
            Some(Due::Lost) => Err((Stop::Lost, None)),
            None => Ok(()),
        }
    }

    async fn text(&mut self, text: &str) -> Step {
        self.socket
            .send_text(text)
            .await
            .map_err(|_| (Stop::Lost, None))?;
        self.pace.sent(Instant::now());
        Ok(())
    }
}
