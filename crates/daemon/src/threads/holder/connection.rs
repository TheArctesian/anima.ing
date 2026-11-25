use std::sync::{Arc, Mutex};

use tokio::io::{AsyncBufReadExt as _, AsyncReadExt as _, BufReader};

use super::{Shared, remote_code};
use crate::threads::wire::{self, Action, Command, MAX_FRAME, Outcome};

pub(super) async fn serve_commands(
    shared: &Arc<Mutex<Shared>>,
    reading: tokio::net::unix::OwnedReadHalf,
    (me, turn): (u64, &Arc<tokio::sync::Mutex<()>>),
) -> bool {
    let mut reader = BufReader::new(reading).take(u64::MAX);
    let mut line = Vec::new();
    let mut quitting = false;
    let mut remote = None;
    loop {
        line.clear();
        reader.set_limit(u64::try_from(MAX_FRAME).unwrap_or(u64::MAX) + 1);
        match reader.read_until(b'\n', &mut line).await {
            Ok(0) | Err(_) => break,
            Ok(_) => {}
        }
        let Some(command) = wire::parse::<Command>(line.trim_ascii_end()) else {
            break;
        };
        let ends = matches!(command.action, Action::Quit);
        let one = Arc::clone(turn).lock_owned().await;
        if !shared.lock().is_ok_and(|state| state.serves(me)) {
            break;
        }
        if let Action::RemoteCode { frame } = command.action {
            if let Ok(frame) = serde_json::from_str(&frame) {
                remote_code::run(shared, (&mut remote, me), frame).await;
            }
            continue;
        }
        let outcome = crate::threads::commands::run(shared, command, one).await;
        if ends && outcome == Outcome::Ok {
            // A command after the Quit would start a program in a holder that ends.
            quitting = true;
            break;
        }
    }
    quitting
}
