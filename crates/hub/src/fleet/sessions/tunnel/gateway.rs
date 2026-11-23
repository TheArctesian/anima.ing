//! guacd's instructions after `ready`, filtered by the session's transfer policy (K4 Sections 4 and 5).
//! guacd holds a download until the client's first `ack`, so a withheld one gets a refusing `ack`
//! (guacamole-server 1.5.5 and 1.6.0 `src/protocols/rdp/download.c`, `guac_rdp_download_ack_handler`).

use guac_protocol::Instruction;

use super::decimal::decimal;
use super::ending::Cause;
use super::relay::{GatewayStep, MAX_STREAMS, Relay};

/// `GUAC_PROTOCOL_STATUS_CLIENT_UNAUTHORIZED` (1.5.5 `src/libguac/guacamole/protocol-types.h:135`).
const CLIENT_UNAUTHORIZED: u32 = 0x0301;
/// `GUAC_PROTOCOL_STATUS_CLIENT_FORBIDDEN` (1.5.5 `src/libguac/guacamole/protocol-types.h:141`).
const CLIENT_FORBIDDEN: u32 = 0x0303;
/// `GUAC_PROTOCOL_STATUS_SERVER_ERROR` (1.5.5 `src/libguac/guacamole/protocol-types.h:59`).
const SERVER_ERROR: u32 = 0x0200;
const STATUS_DIGITS: usize = 5;
const STREAM_DIGITS: usize = 5;
/// libguac `GUAC_USER_STREAM_INDEX_MIMETYPE`: a shared-drive folder listing.
const LISTING: &str = "application/vnd.glyptodon.guacamole.stream-index+json";
/// guacd's instructions that open a stream to the client, with the argument that holds the
/// stream index (guacamole-server 1.5.5 and 1.6.0 `src/libguac/protocol.c`).
const STREAM_OPENINGS: [(&str, usize); 8] = [
    ("img", 0),
    ("audio", 0),
    ("video", 0),
    ("clipboard", 0),
    ("file", 0),
    ("pipe", 0),
    ("argv", 0),
    ("body", 1),
];
const DOWNLOAD_REFUSED: &str = "Download is turned off for this device.";
/// The only `error` text the client gets; any other guacd text stays with the hub (D-11).
const ERROR_TEXT: &str = "Aborted. See logs.";

/// Tells guacamole-common-js the tunnel is open (K4 Section 4, WP-08 H-7).
/// `tunnel_id` is a new UUID, never guacd's connection ID (GW-014).
pub(super) fn tunnel_opened(tunnel_id: &str) -> Instruction {
    Instruction::new("", [tunnel_id])
}

pub(super) fn certificate_changed() -> Instruction {
    Instruction::new(
        "error",
        [
            "Certificate changed".to_owned(),
            CLIENT_FORBIDDEN.to_string(),
        ],
    )
}

impl Relay {
    pub(super) fn on_gateway(&mut self, i: Instruction) -> GatewayStep {
        self.forget_reopened(&i);
        match i.opcode.as_str() {
            "required" => {
                self.note_prompts(&i.args);
                GatewayStep::Forward(i)
            }
            "disconnect" => GatewayStep::End {
                send: i,
                cause: Cause::RemoteClosed,
            },
            "error" => {
                let status = error_status(&i);
                GatewayStep::End {
                    send: Instruction::new("error", [ERROR_TEXT.to_owned(), status.to_string()]),
                    cause: error_cause(status),
                }
            }
            // Internal instructions belong to the hub-client tunnel; guacd's could pass for the hub's (K4 Section 4).
            "" => GatewayStep::Drop,
            "clipboard" if !self.policy.transfer.copy => self.withhold(&i, 0, false),
            "file" if !self.policy.downloads() => self.withhold(&i, 0, true),
            "body" if !self.may_send_body(&i) => self.withhold(&i, 1, true),
            "blob" => self.on_gateway_stream(i, false),
            "end" => self.on_gateway_stream(i, true),
            _ => GatewayStep::Forward(i),
        }
    }

    fn note_prompts(&mut self, names: &[String]) {
        let prompts = self.policy.prompts();
        for name in names {
            if let Some(prompt) = prompts.iter().find(|prompt| **prompt == name.as_str()) {
                self.prompted.insert(prompt);
            }
        }
    }

    /// `body,<object>,<stream>,<mimetype>,<name>`: a folder listing needs the shared drive, a file needs download.
    fn may_send_body(&self, i: &Instruction) -> bool {
        let listing = i.args.get(2).is_some_and(|mimetype| mimetype == LISTING);
        if listing {
            self.policy.shared_drive
        } else {
            self.policy.downloads()
        }
    }

    /// guacd frees a refused download without an `end` and reuses its index, so a new opening clears the withhold.
    fn forget_reopened(&mut self, i: &Instruction) {
        let opened = STREAM_OPENINGS
            .iter()
            .find(|(opcode, _)| *opcode == i.opcode)
            .and_then(|(_, at)| i.args.get(*at))
            .and_then(|index| stream_index(index));
        if let Some(stream) = opened {
            self.withheld_streams.remove(&stream);
        }
    }

    /// An index that is not a stream index is not tracked: the client never saw that stream open.
    fn withhold(&mut self, i: &Instruction, at: usize, refuse: bool) -> GatewayStep {
        let Some(stream) = i.args.get(at).and_then(|index| stream_index(index)) else {
            return GatewayStep::Drop;
        };
        self.withheld_streams.insert(stream);
        if refuse {
            let status = CLIENT_FORBIDDEN.to_string();
            let ack = [stream.to_string(), DOWNLOAD_REFUSED.to_owned(), status];
            GatewayStep::Refuse(Instruction::new("ack", ack))
        } else {
            GatewayStep::Drop
        }
    }

    /// While any stream is withheld, a non-numeric index is dropped: guacamole-common-js reads it as a number.
    fn on_gateway_stream(&mut self, i: Instruction, closes: bool) -> GatewayStep {
        let withheld = match i.args.first().and_then(|index| stream_index(index)) {
            Some(stream) if closes => self.withheld_streams.remove(&stream),
            Some(stream) => self.withheld_streams.contains(&stream),
            None => !self.withheld_streams.is_empty(),
        };
        if withheld {
            GatewayStep::Drop
        } else {
            GatewayStep::Forward(i)
        }
    }
}

fn stream_index(text: &str) -> Option<u16> {
    let index = decimal(text, STREAM_DIGITS)?;
    u16::try_from(index)
        .ok()
        .filter(|index| *index < MAX_STREAMS)
}

fn error_status(i: &Instruction) -> u32 {
    i.args
        .get(1)
        .and_then(|status| decimal(status, STATUS_DIGITS))
        .unwrap_or(SERVER_ERROR)
}

fn error_cause(status: u32) -> Cause {
    match status {
        CLIENT_UNAUTHORIZED | CLIENT_FORBIDDEN => Cause::AuthFailed,
        _ => Cause::GatewayError,
    }
}
