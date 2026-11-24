//! Which client instructions reach guacd (GW-011, GW-012, GW-013, GW-015, D-8).
//! Only allow-listed opcodes with at least the arguments guacd reads pass; anything else closes the tunnel with 1002.
//! libguac indexes `argv` without checking `argc` (guacamole-server 1.5.5, 1.6.0 `src/libguac/user-handlers.c`).
//! The policy narrows the list (K4 Section 4), but a refused paste is dropped, not closed: guacamole-common-js sends the clipboard on its own.

use guac_protocol::{Display, Instruction};

use crate::fleet::sessions::Desktop;

use super::decimal::decimal;
use super::ending::Cause;
use super::relay::{ClientStep, MAX_STREAMS, Relay};

/// Always allowed (GW-012, K4 Section 4); only guacd's terminal protocols apply it.
const COLOR_SCHEME: &str = "color-scheme";
/// Most digits in a `size` value: 8192 has four; one more allows a leading zero.
const SIZE_DIGITS: usize = 5;
const NUMBER_DIGITS: usize = 5;

const INPUT_ARITY: [(&str, usize); 5] = [
    ("key", 2),
    ("mouse", 3),
    ("touch", 7),
    ("sync", 1),
    ("nop", 0),
];

impl Relay {
    pub(super) fn on_client(&mut self, i: Instruction) -> ClientStep {
        match i.opcode.as_str() {
            "" => internal(i),
            "size" => size(&i),
            "display" if self.policy.desktop == Desktop::Vnc => match i.args.as_slice() {
                [choice] if choice == "all" || decimal(choice, 10).is_some() => {
                    ClientStep::Forward(i)
                }
                _ => ClientStep::Drop,
            },
            "disconnect" => ClientStep::End {
                forward: Some(i),
                cause: Cause::OperatorClosed,
            },
            "ack" => ack(i),
            "clipboard" => self.clipboard(i),
            "argv" => self.argv(i),
            "audio" if self.policy.microphone => self.open_stream(i, 2, 0),
            "file" if self.policy.uploads() => self.open_stream(i, 3, 0),
            "put" if self.policy.uploads() && object(&i) => self.open_stream(i, 4, 1),
            "get" if self.policy.shared_drive && object(&i) && i.args.len() >= 2 => {
                ClientStep::Forward(i)
            }
            "blob" => self.on_stream(i, 2, false),
            "end" => self.on_stream(i, 1, true),
            opcode => match INPUT_ARITY.iter().find(|(name, _)| *name == opcode) {
                Some(&(_, arity)) if i.args.len() >= arity => ClientStep::Forward(i),
                _ => invalid(),
            },
        }
    }

    /// A credential `argv` passes once, only after guacd asked for it with `required` (GW-015).
    fn argv(&mut self, i: Instruction) -> ClientStep {
        let Some(name) = i.args.get(2) else {
            return invalid();
        };
        let allowed = name == COLOR_SCHEME
            || self
                .policy
                .prompts()
                .iter()
                .find(|prompt| **prompt == name.as_str())
                .is_some_and(|prompt| self.prompted.remove(prompt));
        if allowed {
            self.open_stream(i, 3, 0)
        } else {
            invalid()
        }
    }

    fn clipboard(&mut self, i: Instruction) -> ClientStep {
        if self.policy.transfer.paste {
            return self.open_stream(i, 2, 0);
        }
        match stream_index(&i, 2, 0) {
            Some(stream) => {
                self.client_streams.remove(&stream);
                self.dropped_client_streams.insert(stream);
                ClientStep::Drop
            }
            None => invalid(),
        }
    }

    fn open_stream(&mut self, i: Instruction, arity: usize, at: usize) -> ClientStep {
        match stream_index(&i, arity, at) {
            Some(stream) => {
                self.dropped_client_streams.remove(&stream);
                self.client_streams.insert(stream);
                ClientStep::Forward(i)
            }
            None => invalid(),
        }
    }

    fn on_stream(&mut self, i: Instruction, arity: usize, closes: bool) -> ClientStep {
        let Some(stream) = stream_index(&i, arity, 0) else {
            return invalid();
        };
        let (open, dropped) = if closes {
            (
                self.client_streams.remove(&stream),
                self.dropped_client_streams.remove(&stream),
            )
        } else {
            (
                self.client_streams.contains(&stream),
                self.dropped_client_streams.contains(&stream),
            )
        };
        match (open, dropped) {
            (true, _) => ClientStep::Forward(i),
            (false, true) => ClientStep::Drop,
            (false, false) => invalid(),
        }
    }
}

/// The internal ping is answered here and never reaches guacd (GW-011).
fn internal(i: Instruction) -> ClientStep {
    if i.args.first().is_some_and(|first| first == "ping") {
        ClientStep::Reply(i)
    } else {
        ClientStep::Drop
    }
}

/// Bounds per K4 Section 4 and R-018; guacamole-common-js sends no DPI (WP-08 H-6).
fn size(i: &Instruction) -> ClientStep {
    let numbers: Option<Vec<u32>> = i.args.iter().map(|arg| decimal(arg, SIZE_DIGITS)).collect();
    let sides = Display::MIN_SIDE_PX..=Display::MAX_SIDE_PX;
    let dpi = Display::MIN_DPI..=Display::MAX_DPI;
    let fits = match numbers.as_deref() {
        Some([width, height]) => sides.contains(width) && sides.contains(height),
        Some([width, height, density]) => {
            sides.contains(width) && sides.contains(height) && dpi.contains(density)
        }
        _ => false,
    };
    match numbers {
        Some(numbers) if fits => {
            ClientStep::Forward(Instruction::new("size", numbers.iter().map(u32::to_string)))
        }
        _ => ClientStep::Drop,
    }
}

/// guacd ignores an `ack` for a stream it has not opened, so only the numbers are checked.
fn ack(i: Instruction) -> ClientStep {
    let numbers = i.args.len() >= 3
        && decimal(&i.args[0], NUMBER_DIGITS).is_some()
        && decimal(&i.args[2], NUMBER_DIGITS).is_some();
    if numbers {
        ClientStep::Forward(i)
    } else {
        invalid()
    }
}

fn stream_index(i: &Instruction, arity: usize, at: usize) -> Option<u16> {
    if i.args.len() < arity {
        return None;
    }
    let index = decimal(i.args.get(at)?, NUMBER_DIGITS)?;
    u16::try_from(index)
        .ok()
        .filter(|index| *index < MAX_STREAMS)
}

fn object(i: &Instruction) -> bool {
    i.args
        .first()
        .is_some_and(|object| decimal(object, NUMBER_DIGITS).is_some())
}

fn invalid() -> ClientStep {
    ClientStep::End {
        forward: None,
        cause: Cause::InvalidInstruction,
    }
}
