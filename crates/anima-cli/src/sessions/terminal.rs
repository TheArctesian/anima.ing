//! `anima ssh` (K4 Section 8), login token only (INT-003): the hub holds the credential and pins
//! the host key; this program only relays bytes.

pub mod codec;
pub mod console;
pub mod ending;
pub mod link;

use std::io;

use link::Streams;

use crate::devices::get::resolve;
use crate::failure::Failure;
use crate::hub::HubError;
use crate::invocation::Invocation;
use crate::raw_mode::RawMode;
use crate::sessions::start::{end, start_ssh};

#[derive(Debug, clap::Args)]
pub struct SshArgs {
    /// The device's ID or its exact name.
    device: String,
}

impl SshArgs {
    /// # Errors
    /// [`Failure`] for a refusal, a changed host key or a lost connection.
    pub async fn run(self, invocation: &Invocation) -> Result<(), Failure> {
        let hub = invocation.hub()?;
        let device_id = resolve(&hub, &self.device).await?;
        let interactive = invocation.stdin_terminal();
        let started = start_ssh(&hub, device_id, console::pixels(console::size())).await?;
        if started.wake_sent {
            invocation.output.note(
                "The device was asleep; a wake packet was sent. Opening can take up to two minutes.",
            );
        }
        let session = started.session.clone();
        let wait = started.opening_wait();
        let mut socket = match hub.open_tunnel(started.ticket, wait).await {
            Ok(socket) => socket,
            Err(error) => {
                let _ended = end(&hub, &session).await;
                return Err(HubError::from(error).into());
            }
        };
        let raw = if interactive {
            Some(RawMode::enter().map_err(|error| Failure::Input {
                what: "the terminal".to_owned(),
                reason: error.to_string(),
            })?)
        } else {
            None
        };
        let mut stdout = io::stdout();
        let streams = Streams {
            keys: console::keys(),
            sizes: console::sizes(interactive),
            output: &mut stdout,
            stop: console::stop(),
        };
        let ending = link::relay(&mut socket, streams).await;
        drop(raw);
        socket.close().await;
        let recorded = end(&hub, &session).await;
        ending::verdict(recorded, ending, &self.device)
    }
}
